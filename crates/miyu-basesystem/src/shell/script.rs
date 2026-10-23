//! 脚本当工具（施工 X-12 上，`docs/blueprint/skills.md`「脚本」第 2、3 条）：一个脚本一件工具，名字、说明、参数照开头注释
//! （`miyu_policy::script`）。起进程、看门、截输出和 `shell` 同一套（[`super::process`]、[`super::Results`]）。
//!
//! - 照开头的 `#!` 起程序：`#!/usr/bin/env python3` 起 `/usr/bin/env`，参数是 `python3` 和脚本；没有 `#!` 的直接起脚本本身。
//!   Windows 上没有 `/usr/bin/env` 这种：照 `#!` 里的解释器的名字（`env` 后面第一个不带 `-` 的）找，没有 `#!` 的照扩展名找（施工 X-34 起和
//!   包里的程序共用 [`miyu_store::packages::launcher`]）。
//! - 参数是她给的那份 JSON，写进标准输入，不超过 30 KiB 的也交给环境变量 `MIYU_ARGS_JSON`（同旧版；施工 X-37：长的放进环境
//!   变量起都起不来）。
//! - 工作目录是这一轮的工作目录；另给一个能写缓存的目录 `MIYU_SCRIPT_CACHE_DIR`（施工 Y-11 上：会话属主家目录的
//!   `cache/<来处>/<工具名>/`，场所会话多一层场所，进沙盒的放进规格的 `write`；会话外面的调用照旧在系统临时目录 `miyu-scripts/<工具名>`
//!   下面；30 天没碰的清掉，跑完清一次，`sweep.rs`），`PYTHONUTF8=1`（Python 的标准输入、输出照 UTF-8）。别的环境变量照 `shell` 的
//!   挑法。
//! - 调用的事实（施工 X-43，[`miyu_tool::Caller`]）：`MIYU_CALLER_VENUE` 是场所（本机的 `local`、场所编号），`MIYU_CALLER_OWNER`
//!   是这一轮开口的是不是机器主人（`1`、`0`）；会话外面的调用没有这两个。只是事实，不是门：挡场所的是开头的 `Venues`，工具照它
//!   给哪种会话。核心环境里残留的照 `shell` 的挑法本来就去掉了（`MIYU_*` 不传）。
//! - 持久的状态目录 `MIYU_SCRIPT_STATE_DIR`（施工 X-44，[`miyu_store::scripts::state_dir`]）：照会话属主、来处、工具名分，场所会话
//!   另照场所分（施工 X-44 补：几个群都归系统账号），跑之前建好（Unix 上 0700），交真实位置；进沙盒的放进规格的 `write`（数据根
//!   照旧藏着）。建不成的、场所写不成目录名的、会话外面的调用（没有属主）不给，照常跑。
//!   能丢的放缓存目录，要留的（登录态）放这里。
//! - 要交图的拷进另一个目录 `MIYU_SCRIPT_IMAGES_DIR`（施工 X-35，[`images`]）：每次调用一个新的空目录，跑完认得的交成图，
//!   到时被杀的不看，读完删掉。
//! - 等开头写的秒数，到时整组杀掉；退出码不是 0 的算出错。访问类别是执行：照跑命令的规矩进沙盒、问人；进沙盒的，规格里
//!   单独放开脚本这一个文件只读（施工 X-12 下：家目录里放的、自己装的包带的在数据根里，沙盒藏着）。
//! - 这一次在沙盒外跑（施工 5-14 改下，同 `shell`）：登记时参数里多一格布尔 `outside_sandbox`（说明是
//!   `software/basesystem/script/outside-sandbox.txt`），写了 `true` 的照越过沙盒问人；交给脚本之前去掉这一格。脚本自己的参数里
//!   本来就有这一格的不加、不覆盖，照原样交给它，这个脚本就只能在沙盒里跑。

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use miyu_kernel::raw::RawJson;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_policy::script::ScriptHead;
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool, Venues};

use super::process::{self, Ending, Guard};
use super::{Results, env, workdir};
use crate::blocking::blocking;
use crate::common::said;
use crate::load::{self, LoadError, say};

mod images;
mod sweep;

/// 跑之前建的哪一种目录。
#[derive(Clone, Copy)]
enum Kept {
    /// 状态目录（施工 X-44）。
    State,
    /// 缓存目录（施工 Y-11 上）。
    Cache,
}

use images::Folder;

/// 参数交给脚本的环境变量（同旧版）。
const ARGS: &str = "MIYU_ARGS_JSON";

/// 参数至多多长还放进环境变量（施工 X-37，X-37 补改成 30 KiB）：Linux 上一个环境变量最多 128 KiB、Windows 上整条「名字=值」
/// 32767 个字（UTF-16 计），超了起都起不来。按字节算的参数，UTF-16 的字数不会多过字节数：30 KiB 加上 `MIYU_ARGS_JSON=` 离
/// 32767 还有两千多（32 KiB 的 ASCII 参数正好超）。长的只在标准输入里。
const ARGS_IN_ENV: usize = 30 * 1024;

/// 能写缓存的目录交给脚本的环境变量（同旧版）。
const CACHE: &str = "MIYU_SCRIPT_CACHE_DIR";

/// 交图的目录交给脚本的环境变量（施工 X-35）。
const IMAGES: &str = "MIYU_SCRIPT_IMAGES_DIR";

/// 状态目录交给脚本的环境变量（施工 X-44）。
const STATE: &str = "MIYU_SCRIPT_STATE_DIR";

/// 这一次在沙盒外跑的那一格（施工 5-14 改下，同 `shell`）。
const OUTSIDE: &str = "outside_sandbox";

/// 调用的场所交给脚本的环境变量（施工 X-43）。
const CALLER_VENUE: &str = "MIYU_CALLER_VENUE";

/// 这一轮开口的是不是机器主人交给脚本的环境变量（施工 X-43）：`1`、`0`。
const CALLER_OWNER: &str = "MIYU_CALLER_OWNER";

/// 一个脚本。
pub(crate) struct Script {
    spec: Spec,
    /// 开头写的给人看的名字，照语言（施工 9-8 六补）。
    display_name: BTreeMap<String, String>,
    /// 开头写的给哪种会话（`Venues`，施工 X-43）：没写的没有。
    venues: Option<Venues>,
    /// 来处：包的编号，家目录里放的是 `~home`（施工 X-44：状态目录照它分）。
    source: String,
    /// 参数里那一格 `outside_sandbox` 是不是登记时加的（施工 5-14 改下）：脚本自己声明了的不算。
    outside: bool,
    file: PathBuf,
    timeout: u64,
    results: Results,
    timed_out: Template,
    failed: Template,
}

impl Script {
    /// 照开头 `head`、脚本文件 `file` 造，给她看的几句从资源目录 `resources` 读：跑完了的同 `shell`，到时、起不来的另有
    /// `software/basesystem/script/`。
    ///
    /// # Errors
    ///
    /// 参数不是 JSON（读开头时已经查过，不会）；资源目录里哪一句读不出来。
    pub(crate) fn load(
        resources: &Path,
        head: &ScriptHead,
        file: PathBuf,
        source: &str,
    ) -> Result<Script, LoadError> {
        let unreadable = |why: String| LoadError {
            file: file.clone(),
            why,
        };
        let mut parameters: serde_json::Value = serde_json::from_str(&head.parameters)
            .map_err(|error| unreadable(error.to_string()))?;
        let outside = offer_outside(
            &mut parameters,
            load::raw(resources, "script", "outside-sandbox")?,
        );
        let parameters = RawJson::of(&parameters)
            .ok_or_else(|| unreadable("parameters are not JSON".to_string()))?;
        let text = |name: &str, fields: &[&str]| load::text(resources, "script", name, fields);
        Ok(Script {
            spec: Spec {
                name: head.name.clone(),
                description: head.description.clone(),
                parameters,
                access: Access::Execute,
                need_load: head.need_load,
                clicall: head.clicall,
                result: None,
            },
            display_name: head.display_name.clone(),
            venues: head.venues.as_deref().map(Venues::of),
            source: source.to_string(),
            outside,
            timeout: head.timeout,
            results: Results::load(resources)?,
            timed_out: text("timed-out", &["seconds"])?,
            failed: text("failed", &["name", "error"])?,
            file,
        })
    }

    /// 起不来、等不了。
    fn failed(&self, error: &io::Error) -> Done {
        let error = error.to_string();
        Done::error(say(
            &self.failed,
            &[("name", &self.spec.name), ("error", &error)],
        ))
        .said(said("shell/failed").with("error", error))
    }

    /// 这一次的状态目录（施工 X-44）：照会话属主、来处、工具名建好（Unix 上 0700），交真实位置。没有属主、数据根的，建不成的
    /// 没有（建不成的记一行 `WARN`）。
    fn state(&self, call: &Call) -> Option<PathBuf> {
        let (root, ids) = (call.data_root.as_deref()?, call.ids.as_ref()?);
        // 场所会话照场所分（施工 X-44 补）：几个群都归系统账号，不分的话 A 群读得到 B 群的。
        let venue = call.caller.as_ref().map(|caller| &caller.venue);
        let Some(dir) =
            miyu_store::scripts::state_dir(root, &ids.owner, venue, &self.source, &self.spec.name)
        else {
            tracing::warn!(target: "miyu::shell", tool = self.spec.name.as_str(), error = "venue too long", "script state not prepared");
            return None;
        };
        self.prepared(&dir, Kept::State)
    }

    /// 这一次的缓存目录（施工 Y-11 上）：会话属主家目录的 `cache/<来处>/<工具名>/`，场所会话多一层场所，建好交真实位置。没有属主、
    /// 数据根的（会话外面的调用），场所写不成目录名的、建不成的照旧在系统临时目录 `miyu-scripts/<工具名>/` 下面。
    fn cache(&self, call: &Call) -> PathBuf {
        let venue = call.caller.as_ref().map(|caller| &caller.venue);
        let home = call
            .data_root
            .as_deref()
            .zip(call.ids.as_ref())
            .and_then(|(root, ids)| {
                miyu_store::scripts::cache_dir(
                    root,
                    &ids.owner,
                    venue,
                    &self.source,
                    &self.spec.name,
                )
            });
        home.and_then(|dir| self.prepared(&dir, Kept::Cache))
            .unwrap_or_else(|| {
                std::env::temp_dir()
                    .join("miyu-scripts")
                    .join(&self.spec.name)
            })
    }

    /// 建好目录 `dir`（缺的上级一起建，Unix 上 0700），交真实位置；建不成的照 `kept` 记一行 `WARN`，没有。
    fn prepared(&self, dir: &Path, kept: Kept) -> Option<PathBuf> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        match builder.create(dir).and_then(|()| dir.canonicalize()) {
            Ok(real) => Some(real),
            Err(error) => {
                let tool = self.spec.name.as_str();
                // 两种各一句写死的话（运行日志不插值，LG-1）。
                match kept {
                    Kept::State => {
                        tracing::warn!(target: "miyu::shell", tool, error = %error, "script state not prepared")
                    }
                    Kept::Cache => {
                        tracing::warn!(target: "miyu::shell", tool, error = %error, "script cache not prepared")
                    }
                }
                None
            }
        }
    }

    /// 起它的命令：照 [`launcher`] 找程序，带了沙盒的经它的助手起；`images` 是交图的目录，`state` 是状态目录，建不成的没有。
    ///
    /// [`launcher`]: miyu_store::packages::launcher
    fn command(
        &self,
        call: &Call,
        (cache, args_json): (&Path, &str),
        (images, state): (Option<&Path>, Option<&Path>),
    ) -> io::Result<std::process::Command> {
        // 是链接的照它指到的真实位置起（施工 X-37）：沙盒只放开那一个文件，链接本身在藏着的数据根里，照它起解释器读不到。
        let file = match std::fs::symlink_metadata(&self.file) {
            Ok(metadata) if metadata.file_type().is_symlink() => std::fs::canonicalize(&self.file)?,
            _ => self.file.clone(),
        };
        let first = miyu_store::packages::first_line(&file)?;
        let launch = miyu_store::packages::launcher(&file, &first, cfg!(windows));
        let (program, args) = (launch.program.into_os_string(), launch.args);
        let (program, args) = match call.sandbox.as_deref() {
            None => (program, args),
            Some(sandboxed) => {
                // 脚本可能在数据根里（家目录里放的、自己装的包带的），沙盒藏着那一片：单独放开它这一个文件只读，解释器才
                // 读得到（施工 X-12 下）。旁边的照旧藏着。
                let mut sandboxed = sandboxed.clone();
                sandboxed.spec.reveal.push(file.clone());
                // 自己的状态目录能写（施工 X-44）、缓存目录能写（施工 Y-11 上）：在藏着的数据根里，越深的越算数。
                sandboxed.spec.write.extend(state.map(Path::to_path_buf));
                sandboxed.spec.write.push(cache.to_path_buf());
                // 交图的目录在缓存那一格旁边（同一个来处下面）：缓存挪进数据根以后它也被藏着，单独放开写（施工 Y-11 上）。
                sandboxed.spec.write.extend(images.map(Path::to_path_buf));
                miyu_sandbox::argv(&sandboxed, Path::new(&program), &args)
                    .map(|(helper, args)| (helper.into_os_string(), args))
                    .map_err(io::Error::other)?
            }
        };
        let mut vars = env::passed(std::env::vars_os(), call.sandbox.is_some());
        vars.extend(env::told(call.caller.as_ref()));
        if args_json.len() <= ARGS_IN_ENV {
            vars.push((ARGS.into(), args_json.into()));
        }
        vars.push((CACHE.into(), cache.as_os_str().to_owned()));
        if let Some(images) = images {
            vars.push((IMAGES.into(), images.as_os_str().to_owned()));
        }
        if let Some(state) = state {
            vars.push((STATE.into(), state.as_os_str().to_owned()));
        }
        if let Some(caller) = &call.caller {
            vars.push((CALLER_VENUE.into(), caller.venue.as_str().into()));
            let owner = if caller.owner { "1" } else { "0" };
            vars.push((CALLER_OWNER.into(), owner.into()));
        }
        // Python 的标准输入、输出照 UTF-8：Windows 上默认照本地的编码，中文的参数、输出会乱（输出照 UTF-8 解）。
        vars.push(("PYTHONUTF8".into(), "1".into()));
        let mut command = std::process::Command::new(program);
        command
            .args(args)
            .current_dir(workdir(call))
            .env_clear()
            .envs(vars);
        if let Some(sandboxed) = call.sandbox.as_deref() {
            command.envs(sandboxed.env.iter().map(|(name, value)| (name, value)));
        }
        Ok(command)
    }
}

impl Script {
    /// 交给脚本的参数（施工 5-14 改下）：登记时加了 `outside_sandbox` 的去掉这一格，脚本只拿到自己声明的；读不懂的、没写这一格的
    /// 照原样。
    fn passed(&self, call: &Call) -> String {
        if !self.outside {
            return call.args.clone();
        }
        match serde_json::from_str::<serde_json::Value>(&call.args) {
            Ok(serde_json::Value::Object(mut args)) => match args.remove(OUTSIDE) {
                Some(_) => serde_json::Value::Object(args).to_string(),
                None => call.args.clone(),
            },
            _ => call.args.clone(),
        }
    }
}

/// 参数 `parameters` 里加上 `outside_sandbox`（说明是 `description`）：是对象、自己没声明这一格的才加（`properties` 没有的建上），
/// 交回加了没有。
fn offer_outside(parameters: &mut serde_json::Value, description: String) -> bool {
    let Some(object) = parameters.as_object_mut() else {
        return false;
    };
    let properties = object
        .entry("properties")
        .or_insert_with(|| serde_json::json!({}));
    match properties.as_object_mut() {
        Some(properties) if !properties.contains_key(OUTSIDE) => {
            properties.insert(
                OUTSIDE.to_string(),
                serde_json::json!({"type": "boolean", "description": description}),
            );
            true
        }
        _ => false,
    }
}

impl Tool for Script {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn display_name(&self) -> BTreeMap<String, String> {
        self.display_name.clone()
    }

    /// 开头写了 `Venues` 的照它给哪种会话（施工 X-43）；没写的照工具面自己的规矩，哪种会话都有。
    fn venues(&self) -> Option<Venues> {
        self.venues
    }

    /// 脚本文件（施工 X-37 补）：放行出沙盒时照它判那一刻的真实位置。
    fn file(&self) -> Option<&Path> {
        Some(&self.file)
    }

    fn asking(&self, call: &Call) -> Vec<(&'static str, String)> {
        vec![
            ("script", self.spec.name.clone()),
            ("args", self.passed(call)),
        ]
    }

    /// 这次调用要在沙盒外跑（施工 5-14 改下）：登记时加了那一格、参数里写了 `true` 的。
    fn outside_sandbox(&self, call: &Call) -> bool {
        self.outside
            && serde_json::from_str::<serde_json::Value>(&call.args)
                .is_ok_and(|args| args.get(OUTSIDE) == Some(&serde_json::Value::Bool(true)))
    }

    fn run(&self, call: Call, progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let cache = self.cache(&call);
            let args_json = self.passed(&call);
            let command = std::fs::create_dir_all(&cache).map(|()| Folder::create(&cache));
            let (command, folder) = match command.and_then(|folder| {
                let state = self.state(&call);
                let dirs = (folder.as_ref().map(Folder::path), state.as_deref());
                let command = self.command(&call, (&cache, &args_json), dirs)?;
                Ok((command, folder))
            }) {
                Ok(made) => made,
                Err(error) => return self.failed(&error),
            };
            let input = Some(args_json.into_bytes());
            let started =
                match process::start(command, input, move |text| progress.push(text), true) {
                    Ok(started) => started,
                    Err(error) => return self.failed(&error),
                };
            // 起来了就看着：这次调用被叫停（future 被丢掉）时，整组杀掉。
            let guard = Guard::new(started.group());
            let wait = Duration::from_secs(self.timeout);
            let name = self.spec.name.clone();
            // 收图也在阻塞线程里：读文件、删目录。到时被杀的不看，目录照样删（丢掉时）。
            let finished = blocking(call.stop.clone(), move |_| {
                let finished = started.wait(wait)?;
                let pictures = match folder {
                    Some(folder) if !matches!(finished.ending, Ending::TimedOut) => {
                        folder.collect(&name)
                    }
                    _ => Vec::new(),
                };
                Ok::<_, io::Error>((finished, pictures))
            })
            .await;
            guard.disarm();
            // 跑完清一次这个人的缓存（施工 Y-11 上）：照时间，一分钟内只清一次，不等它。
            sweep::after(&call);
            let seconds = self.timeout.to_string();
            match finished {
                Ok((finished, pictures)) => pictures.into_iter().fold(
                    self.results.ended(finished, || {
                        (
                            say(&self.timed_out, &[("seconds", &seconds)]),
                            seconds.clone(),
                        )
                    }),
                    Done::image,
                ),
                Err(error) => self.failed(&error),
            }
        })
    }
}
