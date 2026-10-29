//! `miyu web`（`docs/blueprint/web-module.md`「怎么走」第十一条第 1 款，施工 W-9）：主程序里不放网页的代码，照清单找网页
//! 软件（施工 9-3：子命令是 `web` 的那个包的程序，出厂那一份是 `miyu-web`），找包文件夹的 `bin/`、主程序真实位置旁边的
//! （Windows 上加 `.exe`，施工 F-8 上再补），把参数原样交给它的 `open`，等它退出，退出码照它的。没装的说怎么装，退出码 1
//! （CL-2 下照设计 47：报错 `miyu: ` 打头，装法写成提示，照软件源装）。

#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;

use miyu_store::env::Env;
use miyu_store::packages::{Packages, locate};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::diagnostic::{self, Problem};
use crate::language::{self, Language};

/// 清单里找不到网页那一份时照它找：出厂的网页软件。
const PROGRAM: &str = "miyu-web";

/// `miyu web` 的参数：照原样交给网页软件。给人看的说明在帮助页里（[`crate::help`]）。
#[derive(Debug, Clone, Default, Args)]
pub struct Web {
    /// 网页软件这一次在哪个端口上听（没在跑时）。
    #[arg(long)]
    pub port: Option<u16>,
    /// 不开浏览器，印网址。
    #[arg(long)]
    pub print: bool,
    /// 忘了密码：带一次性码打开网页重设。
    #[arg(long)]
    pub reset: bool,
    /// 作废全部浏览器的登录。
    #[arg(long)]
    pub logout: bool,
    /// 打开时直接到「软件后台」里这个软件的页面（施工 F-6 下）。
    #[arg(long, value_name = "ID")]
    pub package: Option<String>,
}

impl Web {
    /// 交给 `miyu-web open` 的参数。
    fn args(&self) -> Vec<String> {
        let mut args = vec!["open".to_string()];
        if let Some(port) = self.port {
            args.extend(["--port".to_string(), port.to_string()]);
        }
        for (on, flag) in [
            (self.print, "--print"),
            (self.reset, "--reset"),
            (self.logout, "--logout"),
        ] {
            if on {
                args.push(flag.to_string());
            }
        }
        if let Some(package) = &self.package {
            args.extend(["--package".to_string(), package.clone()]);
        }
        args
    }
}

/// 跑一次 `miyu web`：照两层的清单找网页软件。
pub fn web(args: Web) -> ExitCode {
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    let (program, dir) = program(&Env::current());
    ExitCode::from(web_on(
        &args,
        &main,
        &dir,
        &program,
        language::current(),
        &mut io::stderr(),
    ))
}

/// 网页软件的程序名和包文件夹：两层清单里子命令是 `web` 的那一份的（施工 9-3）；没有的照出厂的 [`PROGRAM`]，包文件夹是空的。
fn program(env: &Env) -> (String, PathBuf) {
    let fallback = || (PROGRAM.to_string(), PathBuf::new());
    let (Ok(root), Ok(resources)) = (DataRoot::locate(env), ResourceRoot::locate(env)) else {
        return fallback();
    };
    Packages::new(&resources, &root)
        .read()
        .into_iter()
        .find_map(|found| {
            let dir = found.files_dir();
            let manifest = found.read.ok()?;
            let command = manifest.command_named("web")?;
            Some((command.program.clone(), dir))
        })
        .unwrap_or_else(fallback)
}

/// 同 [`web`]：主程序在 `main`，网页软件叫 `program`、包文件夹在 `dir`，没装时照 `language` 说在 `err` 上。交回退出码。
pub fn web_on(
    args: &Web,
    main: &Path,
    dir: &Path,
    program: &str,
    language: Language,
    err: &mut dyn Write,
) -> u8 {
    let Some(launch) = locate(program, dir, main) else {
        diagnostic::error(err, language, &not_installed(language));
        return 1;
    };
    // 照 `launch` 起：Windows 上照 `#!` 起的是解释器加脚本（施工 X-34）。主程序的真实位置照转交的规矩交（施工 G-15）：装进
    // 数据根的网页软件旁边没有主程序，要拉起核心的照它找；包文件夹有的也交。
    let mut command = launch.command();
    command.args(args.args()).env(
        miyu_store::env::PROGRAM,
        std::fs::canonicalize(main).unwrap_or_else(|_| main.to_path_buf()),
    );
    if !dir.as_os_str().is_empty() {
        command.env("MIYU_PACKAGE_DIR", dir);
    }
    match command.status() {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1),
        Err(error) => {
            let program = launch.program.display();
            let problem = match language {
                Language::Chinese => Problem::new(format!("无法运行 {program}：{error}")),
                Language::English => Problem::new(format!("cannot run {program}: {error}")),
            };
            diagnostic::error(err, language, &problem);
            1
        }
    }
}

/// 网页界面的包编号：没装时照它说怎么装（同没装终端界面的那一句，`head.rs`）。
const PACKAGE: &str = "web";

/// 没装网页界面：照软件源装。
fn not_installed(language: Language) -> Problem {
    match language {
        Language::Chinese => {
            Problem::new("未安装网页界面").hint(format!("用 'miyu pkg install {PACKAGE}' 安装"))
        }
        Language::English => Problem::new("no web interface installed")
            .hint(format!("install it with 'miyu pkg install {PACKAGE}'")),
    }
}
