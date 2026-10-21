//! 脚本当工具（施工 X-12 上，`docs/blueprint/skills.md`「脚本」第 2、3 条）：一个脚本一件工具，名字、说明、参数照开头注释
//! （`miyu_policy::script`）。起进程、看门、截输出和 `shell` 同一套（[`super::process`]、[`super::Results`]）。
//!
//! - 照开头的 `#!` 起程序：`#!/usr/bin/env python3` 起 `/usr/bin/env`，参数是 `python3` 和脚本；没有 `#!` 的直接起脚本本身。
//!   Windows 上没有 `/usr/bin/env` 这种：照 `#!` 里的解释器的名字（`env` 后面那个）找，没有 `#!` 的照扩展名找（施工 X-34 起和
//!   包里的程序共用 [`miyu_store::packages::launcher`]）。
//! - 参数是她给的那份 JSON，交给环境变量 `MIYU_ARGS_JSON`，也写进标准输入（同旧版）。
//! - 工作目录是这一轮的工作目录；另给一个能写缓存的目录 `MIYU_SCRIPT_CACHE_DIR`（系统临时目录下 `miyu-scripts/<工具名>`，
//!   沙盒的工作区那一级本来就能写临时目录），`PYTHONUTF8=1`（Python 的标准输入、输出照 UTF-8）。别的环境变量照 `shell` 的
//!   挑法。
//! - 要交图的拷进另一个目录 `MIYU_SCRIPT_IMAGES_DIR`（施工 X-35，[`images`]）：每次调用一个新的空目录，跑完认得的交成图，
//!   到时被杀的不看，读完删掉。
//! - 等开头写的秒数，到时整组杀掉；退出码不是 0 的算出错。访问类别是执行：照跑命令的规矩进沙盒、问人；进沙盒的，规格里
//!   单独放开脚本这一个文件只读（施工 X-12 下：家目录里放的、自己装的包带的在数据根里，沙盒藏着）。

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_policy::script::ScriptHead;
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool};

use super::process::{self, Ending, Guard};
use super::{Results, env, workdir};
use crate::blocking::blocking;
use crate::common::said;
use crate::load::{self, LoadError, say};

mod images;

use images::Folder;

/// 参数交给脚本的环境变量（同旧版）。
const ARGS: &str = "MIYU_ARGS_JSON";

/// 能写缓存的目录交给脚本的环境变量（同旧版）。
const CACHE: &str = "MIYU_SCRIPT_CACHE_DIR";

/// 交图的目录交给脚本的环境变量（施工 X-35）。
const IMAGES: &str = "MIYU_SCRIPT_IMAGES_DIR";

/// 一个脚本。
pub(crate) struct Script {
    spec: Spec,
    /// 开头写的给人看的名字，照语言（施工 9-8 六补）。
    display_name: BTreeMap<String, String>,
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
    ) -> Result<Script, LoadError> {
        let parameters = serde_json::from_str(&head.parameters).map_err(|error| LoadError {
            file: file.clone(),
            why: error.to_string(),
        })?;
        let text = |name: &str, fields: &[&str]| load::text(resources, "script", name, fields);
        Ok(Script {
            spec: Spec {
                name: head.name.clone(),
                description: head.description.clone(),
                parameters,
                access: Access::Execute,
                need_load: head.need_load,
                clicall: head.clicall,
            },
            display_name: head.display_name.clone(),
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

    /// 起它的命令：照 [`launcher`] 找程序，带了沙盒的经它的助手起；`images` 是交图的目录，建不成的没有。
    ///
    /// [`launcher`]: miyu_store::packages::launcher
    fn command(
        &self,
        call: &Call,
        cache: &Path,
        images: Option<&Path>,
    ) -> io::Result<std::process::Command> {
        let first = miyu_store::packages::first_line(&self.file)?;
        let launch = miyu_store::packages::launcher(&self.file, &first, cfg!(windows));
        let (program, args) = (launch.program.into_os_string(), launch.args);
        let (program, args) = match call.sandbox.as_deref() {
            None => (program, args),
            Some(sandboxed) => {
                // 脚本可能在数据根里（家目录里放的、自己装的包带的），沙盒藏着那一片：单独放开它这一个文件只读，解释器才
                // 读得到（施工 X-12 下）。旁边的照旧藏着。
                let mut sandboxed = sandboxed.clone();
                sandboxed.spec.reveal.push(self.file.clone());
                miyu_sandbox::argv(&sandboxed, Path::new(&program), &args)
                    .map(|(helper, args)| (helper.into_os_string(), args))
                    .map_err(io::Error::other)?
            }
        };
        let mut vars = env::passed(std::env::vars_os(), call.sandbox.is_some());
        vars.push((ARGS.into(), call.args.clone().into()));
        vars.push((CACHE.into(), cache.as_os_str().to_owned()));
        if let Some(images) = images {
            vars.push((IMAGES.into(), images.as_os_str().to_owned()));
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

impl Tool for Script {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn display_name(&self) -> BTreeMap<String, String> {
        self.display_name.clone()
    }

    fn asking(&self, call: &Call) -> Vec<(&'static str, String)> {
        vec![
            ("script", self.spec.name.clone()),
            ("args", call.args.clone()),
        ]
    }

    fn run(&self, call: Call, progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let cache = std::env::temp_dir()
                .join("miyu-scripts")
                .join(&self.spec.name);
            let command = std::fs::create_dir_all(&cache).map(|()| Folder::create(&cache));
            let (command, folder) = match command.and_then(|folder| {
                let command = self.command(&call, &cache, folder.as_ref().map(Folder::path))?;
                Ok((command, folder))
            }) {
                Ok(made) => made,
                Err(error) => return self.failed(&error),
            };
            let input = Some(call.args.clone().into_bytes());
            let started = match process::start(command, input, move |text| progress.push(text)) {
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
