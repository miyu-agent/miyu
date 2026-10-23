//! `miyu service enable|disable|status`（施工 Z-1 下，`docs/designs/12-进程形态与分发.md` 第二节）：替本人装、卸、看开机或登录
//! 就起核心的那一份服务，只管本人这一层（系统级的要 root、要建用户，照 `packaging/README.md` 手做）。
//!
//! - [`Service`]：参数；[`service()`]：跑一次，交回退出码；
//! - 写什么、跑什么在 `plan.rs`（只算不做），怎么走在 `flow.rs`，经「机器」这个接口干活（`machine.rs`）：真的那一台写文件、
//!   跑服务管理器的命令，测试里换替身。
//!
//! 输出是给人看的：跟界面语言，成了印在标准输出上，没成印在标准错误上。一个字都不进模型面。

mod flow;
mod machine;
mod plan;
mod shape;
#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};

use crate::ask::{Format, exit};
use crate::language::{self, Language};

pub(crate) use plan::{Manager, Problem};

/// `miyu service` 的参数。给人看的说明在帮助页里（[`crate::help`]）。
#[derive(Debug, Clone, Args)]
#[command(disable_help_subcommand = true, subcommand_required = true)]
pub struct Service {
    /// 装、卸还是看。
    #[command(subcommand)]
    pub action: ServiceAction,
    /// `text` 给人看（默认），`json` 给头、脚本（施工 Z-1 下补，`cli/service.md`「机读格式」）。
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// 装、卸还是看。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum ServiceAction {
    /// 写好服务、交给服务管理器，开机或登录就起。
    Enable,
    /// 停掉、从服务管理器里拿掉、删掉写的文件。
    Disable,
    /// 装没装、开没开着。
    Status,
}

/// 走完了要说的那一句。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Said {
    /// 装好了：哪种管理器、写在哪。
    Enabled(Manager, PathBuf),
    /// 卸掉了：哪种管理器，删了哪个文件。
    Disabled(Manager, PathBuf),
    /// 本来就没装：哪种管理器，该写在哪。
    NotEnabled(Manager, PathBuf),
    /// 装没装、开没开：`on` 是服务管理器那边开着没有，问不了的没有。
    Status {
        /// 哪种管理器。
        manager: Manager,
        /// 写的文件在哪。
        file: PathBuf,
        /// 文件在不在。
        installed: bool,
        /// 服务管理器那边开着没有。
        on: Option<bool>,
    },
    /// 装不了：算不出来。
    Cannot(Problem),
    /// 不知道家目录在哪。
    NoHome,
    /// 没成：哪一步，系统的原话。
    Failed(String, String),
}

impl Said {
    /// 退出码：成了（连同本来没装、看）是 0，别的是 1（`cli/main.md`「退出码」）。
    pub(crate) fn code(&self) -> u8 {
        match self {
            Said::Enabled(..) | Said::Disabled(..) | Said::NotEnabled(..) | Said::Status { .. } => {
                exit::OK
            }
            Said::Cannot(_) | Said::NoHome | Said::Failed(..) => exit::ERROR,
        }
    }
}

/// 跑一次 `miyu service …`：照界面语言说那一句，交回退出码。
pub fn service(args: Service) -> ExitCode {
    let language = language::current();
    let said = flow::run(&machine::Real, args.action);
    match args.format {
        Format::Text => say(language, &said),
        // 机读的成没成都印在标准输出上：头只读一处（施工 Z-1 下补）。
        Format::Json => {
            if writeln!(io::stdout(), "{}", shape::shaped(&said)).is_err() {
                // 输出关了：没有别处可说。
            }
        }
    }
    ExitCode::from(said.code())
}

/// 说出来：成了的在标准输出上，没成的在标准错误上。
fn say(language: Language, said: &Said) {
    let line = language.service(said);
    let written = match said.code() {
        exit::OK => writeln!(io::stdout(), "{line}"),
        _ => writeln!(io::stderr(), "{line}"),
    };
    if written.is_err() {
        // 输出关了：没有别处可说。
    }
}
