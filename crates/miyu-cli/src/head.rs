//! 打开默认的界面（施工 9-3，`docs/blueprint/cli/main.md`「怎么走」第 3 条，`packages.md`「入口」）：直接敲 `miyu`、`miyu
//! config` 时，连上核心（没在跑就拉起，界面本来就要它）问 `config.get` 拿 `ui.head`，照清单找这个编号的界面包，程序照
//! 9-2 的找法（包文件夹的 `bin/`、`miyu` 旁边的，施工 F-8 上再补），换成它。`miyu config` 带 `--page config`，界面的清单 `[ui] opens` 里没有这一页的不拉起。
//! 只在标准输入、标准输出都是终端时才走到这里（主程序先看）。

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use miyu_config::package::{Manifest, PackageKind};
use miyu_ipc::connect_or_start;
use miyu_store::env::Env;
use miyu_store::packages::{Found, Launch, Packages};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::diagnostic::{self, Problem};
use crate::language::{self, Language};
use crate::link;
use crate::packages;
use crate::rpc::Rpc;

/// 打开的结果。
#[derive(Debug, PartialEq, Eq)]
pub enum Opened {
    /// 拉起了（或者没拉起、说了为什么）：退出码。
    Ran(u8),
    /// 这个界面不认要的那一页：照旧印帮助。
    NoPage,
}

/// 打开 `ui.head` 指的界面，要的是 `page` 那一页（没有的是它的默认页）。拉起核心用 `start`。
pub fn open(page: Option<&str>, start: impl FnOnce() -> Command) -> Opened {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return Opened::Ran(diagnostic::failed(
                language::current(),
                &Problem::new(error.to_string()),
            ));
        }
    };
    let env = Env::current();
    let asked = runtime.block_on(head(&env, start));
    drop(runtime);
    let (head, language) = match asked {
        Ok(asked) => asked,
        // 握手被拒：核心的原话已经印过了。
        Err(None) => return Opened::Ran(1),
        Err(Some(problem)) => {
            return Opened::Ran(diagnostic::failed(language::current(), &problem));
        }
    };
    let found = match (DataRoot::locate(&env), ResourceRoot::locate(&env)) {
        (Ok(root), Ok(resources)) => Packages::new(&resources, &root).read(),
        _ => Vec::new(),
    };
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    match plan(&head, page, &found, &main) {
        Plan::Run(launch, args) => {
            let mut command = launch.command();
            command.args(&args);
            Opened::Ran(packages::run(command, &mut io::stderr()))
        }
        Plan::NoPage => Opened::NoPage,
        Plan::Missing => {
            diagnostic::error(
                &mut io::stderr(),
                language,
                &unavailable(&head, None, &installed(&found, &main), language),
            );
            Opened::Ran(1)
        }
        Plan::NoProgram(program) => {
            diagnostic::error(
                &mut io::stderr(),
                language,
                &unavailable(&head, Some(&program), &installed(&found, &main), language),
            );
            Opened::Ran(1)
        }
    }
}

/// 连上核心，问 `ui.head`；交回它和这个连接说的语言。连不上、被拒、核心没答的交回原因；握手被拒的是空的（核心的原话
/// 已经印过了）。
async fn head(
    env: &Env,
    start: impl FnOnce() -> Command,
) -> Result<(String, Language), Option<Problem>> {
    let failed = |error: &dyn std::fmt::Display| Some(Problem::new(error.to_string()));
    let root = DataRoot::locate(env).map_err(|error| failed(&error))?;
    root.prepare().map_err(|error| failed(&error))?;
    let (connection, token) = connect_or_start(&root, start)
        .await
        .map_err(|error| failed(&error))?;
    let mut rpc = Rpc::new(connection, "head");
    let asked = language::current();
    let hello = link::hello(&mut rpc, &token, &asked, false, &mut io::stderr())
        .await
        .map_err(|_| None)?;
    let language = link::spoken(&hello, asked);
    let reply = rpc
        .call("config.get", json!({"keys": ["ui.head"]}))
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(|| Some(language.disconnected()))?;
    let head = reply["result"]["items"]["ui.head"]["value"]
        .as_str()
        .ok_or_else(|| Some(Problem::new(reply.to_string())))?;
    Ok((head.to_string(), language))
}

/// 照清单定怎么开。
#[derive(Debug, PartialEq, Eq)]
enum Plan {
    /// 照这个起（施工 X-34：Windows 上照 `#!` 起的是解释器加脚本），带这几个参数。
    Run(Launch, Vec<OsString>),
    /// 界面不认那一页。
    NoPage,
    /// 没有这个界面：没有清单、不是界面、没有子命令。
    Missing,
    /// 有清单，程序不在包文件夹的 `bin/`、也不在 `miyu` 旁边：程序名（9-3 补：出厂带了终端的清单，程序随 M9）。
    NoProgram(String),
}

/// 编号是 `head` 的界面包怎么开：要 `page` 那一页的带 `--page <页>`，清单里没认这一页的不开。
fn plan(head: &str, page: Option<&str>, found: &[Found], main: &Path) -> Plan {
    let Some((one, manifest)) = found
        .iter()
        .find(|one| one.id == head)
        .and_then(|one| Some((one, one.read.as_ref().ok()?)))
        .filter(|(_, manifest)| manifest.kind == PackageKind::Ui)
    else {
        return Plan::Missing;
    };
    if let Some(page) = page
        && !opens(manifest, page)
    {
        return Plan::NoPage;
    }
    let Some(command) = manifest.command() else {
        return Plan::Missing;
    };
    let Some(launch) = one.program(&command.program, main) else {
        return Plan::NoProgram(command.program.clone());
    };
    let args = page.map_or_else(Vec::new, |page| {
        vec![OsString::from("--page"), OsString::from(page)]
    });
    Plan::Run(launch, args)
}

/// 界面的清单认不认 `page` 这一页。
fn opens(manifest: &Manifest, page: &str) -> bool {
    manifest
        .ui
        .as_ref()
        .is_some_and(|ui| ui.opens.iter().any(|one| one == page))
}

/// 装了的界面：读成了的、`kind = "ui"`、有子命令、程序在包文件夹的 `bin/` 或 `miyu`（`main`）旁边的，照编号排。只有清单、程序还不在的不算：
/// 列出来也打不开（9-3 补）。
fn installed(found: &[Found], main: &Path) -> Vec<String> {
    found
        .iter()
        .filter(|one| {
            one.read.as_ref().is_ok_and(|manifest| {
                manifest.kind == PackageKind::Ui
                    && manifest
                        .command()
                        .is_some_and(|command| one.program(&command.program, main).is_some())
            })
        })
        .map(|one| one.id.clone())
        .collect()
}

/// 打不开 `head`（设计 47）：没有这个界面（`program` 是空的）、有清单程序没找到；装了的有哪几个。
fn unavailable(
    head: &str,
    program: Option<&str>,
    installed: &[String],
    language: Language,
) -> Problem {
    let chinese = language == Language::Chinese;
    let problem = match (program, installed.is_empty(), chinese) {
        (Some(program), _, true) => {
            Problem::new(format!("界面 '{head}' 的程序 '{program}' 找不到"))
                .hint(format!("用 'miyu pkg install {head}' 重新安装"))
        }
        (Some(program), _, false) => Problem::new(format!(
            "program '{program}' of user interface '{head}' not found"
        ))
        .hint(format!("reinstall it with 'miyu pkg install {head}'")),
        (None, true, true) => {
            Problem::new("没有安装界面").hint(format!("用 'miyu pkg install {head}' 安装"))
        }
        (None, true, false) => Problem::new("no user interface installed")
            .hint(format!("install one with 'miyu pkg install {head}'")),
        (None, false, true) => Problem::new(format!("界面 '{head}' 没有安装"))
            .hint(format!("用 'miyu pkg install {head}' 安装")),
        (None, false, false) => Problem::new(format!("user interface '{head}' is not installed"))
            .hint(format!("install it with 'miyu pkg install {head}'")),
    };
    match (installed.is_empty(), chinese) {
        (true, true) => problem.hint("不用界面也可以：miyu ask \"<消息>\""),
        (true, false) => problem.hint("or chat without one: miyu ask \"<message>\""),
        (false, true) => problem.hint(format!(
            "或者换成已安装的界面（{}）：miyu config set ui.head <编号>",
            installed.join("、")
        )),
        (false, false) => problem.hint(format!(
            "or switch to an installed one ({}): miyu config set ui.head <id>",
            installed.join(", ")
        )),
    }
}
