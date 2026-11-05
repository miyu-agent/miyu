//! `miyu upgrade`（施工 RL-4，设计 `12-进程形态与分发.md` 第五节 R10、`59-发版.md` 第三节第 2 条、第四节）：升级 Miyu 本身，
//! 或者告诉人该用哪个包管理器。
//!
//! - [`Upgrade`]：参数；[`upgrade()`]：跑一次，交回退出码；
//! - 只算不做的在 `plan.rs`（标记、版本、挑发布），怎么走在 `flow.rs`，下载在 `net.rs`（测试换替身），换文件在 `swap.rs`。
//!
//! 输出是给人看的：跟界面语言，成了印在标准输出上，没成印在标准错误上。不连核心，数据根一个字节都不碰。

mod flow;
mod net;
mod plan;
mod swap;
#[cfg(test)]
mod tests;

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;

use crate::ask::exit;
use crate::language::{self, Language};

pub(crate) use net::NetError;
pub(crate) use plan::Channel;

/// `miyu upgrade` 的参数。给人看的说明在帮助页里（[`crate::help`]）。
#[derive(Debug, Clone, Args)]
pub struct Upgrade {
    /// 预发布也算（演练用）。长选项写完整的词，同 GitHub、`gh` 的 prerelease（主人 11-05 定，原来叫 `pre`，不留别名）。
    #[arg(long)]
    pub prerelease: bool,
    /// 不问，直接升。
    #[arg(long)]
    pub yes: bool,
}

/// 问「升不升」的回答。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    /// 升。
    Yes,
    /// 不升。
    No,
    /// 没得答：标准输入关着。
    Closed,
}

/// 走到哪一步了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// 在下载这个压缩包。
    Downloading(String),
    /// 在解开、查新版。
    Checking,
    /// 在换。
    Replacing,
}

/// 走完了要说的那一句。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Said {
    /// 包管理器装的：说用哪条命令。
    PackageManager(Channel),
    /// 没有标记（手工解开的压缩包）：只说去哪下。
    Unmarked(String),
    /// 标记文件读不了、写得不对：在哪、为什么。
    BadMarker(PathBuf, String),
    /// 这一版没编进发版公钥：不自己升级。
    NoKeys,
    /// 这种系统不在发的四种里。
    UnknownSystem,
    /// 没拿到。
    Net(NetError),
    /// 一个能用的发布都没有。
    NoRelease,
    /// 还没有正式版，列表里有预发布（不带 `--prerelease` 时，施工 RL-4补2）。
    OnlyPre,
    /// 已经是最新的：现在这一版。
    UpToDate(String),
    /// 问了，答不升。
    Cancelled,
    /// 要问，没得答。
    Unconfirmed,
    /// 发布里少了这个文件（或者清单里没有它那一行）。
    MissingAsset(String),
    /// 签名验不过：原话。
    BadSignature(String),
    /// 校验和清单写得不对：原话。
    BadSums(String),
    /// 下载的压缩包和清单的哈希对不上：哪一个。
    HashMismatch(String),
    /// 新版里少了这几样。
    BadPackage(Vec<String>),
    /// 新版的 `miyu --version` 报的不对。
    WrongVersion {
        /// 该报的。
        expected: String,
        /// 报的。
        said: String,
    },
    /// 装的目录没有写权限：不提权。
    NotWritable(PathBuf),
    /// 没成：哪一步，系统的原话。
    Failed(String, String),
    /// 升好了：从哪一版到哪一版；删不掉、留着的旧版目录。
    Upgraded {
        /// 原来的版本。
        from: String,
        /// 新的版本。
        to: String,
        /// 留着的旧版目录。
        left: Option<PathBuf>,
        /// 安装目录第一层原样留着的、不是 Miyu 的（施工 RL-4补3）。
        kept: Vec<String>,
    },
}

impl Said {
    /// 退出码：升好了、已经是最新的、照包管理器升、只说下载地址的是 0；别的 1（`cli/main.md`「退出码」）。
    pub(crate) fn code(&self) -> u8 {
        match self {
            Said::Upgraded { .. }
            | Said::UpToDate(_)
            | Said::PackageManager(_)
            | Said::Unmarked(_) => exit::OK,
            _ => exit::ERROR,
        }
    }
}

/// 跑一次 `miyu upgrade`：照界面语言说，交回退出码。
pub fn upgrade(args: Upgrade) -> ExitCode {
    let language = language::current();
    let said = real(&args, language);
    say(language, &said);
    ExitCode::from(said.code())
}

/// 真的那一趟：真的程序位置、网络、编进来的公钥、真的文件。
fn real(args: &Upgrade, language: Language) -> Said {
    let exe = match std::env::current_exe().and_then(|exe| exe.canonicalize()) {
        Ok(exe) => exe,
        Err(error) => return Said::Failed("locate".to_string(), error.to_string()),
    };
    let net = match net::Real::new() {
        Ok(net) => net,
        Err(why) => return Said::Failed("network".to_string(), why),
    };
    let Some(current) = plan::Version::parse(env!("CARGO_PKG_VERSION")) else {
        return Said::Failed("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    };
    let mut ask = |from: &plan::Version, to: &plan::Version| {
        asked(
            &mut io::stdin().lock(),
            &language.upgrade_question(&from.to_string(), &to.to_string()),
        )
    };
    let mut step = |step: Step| {
        if writeln!(io::stdout(), "{}", language.upgrade_step(&step)).is_err() {
            // 输出关了：没有别处可说。
        }
    };
    let mut rename = |from: &Path, to: &Path| std::fs::rename(from, to);
    let mut world = flow::World {
        exe,
        net: &net,
        keys: miyu_source::sign::release(),
        target: miyu_config::package::Target::here().map(miyu_config::package::Target::as_str),
        exe_suffix: std::env::consts::EXE_SUFFIX,
        current,
        version_of: &version_of,
        ask: &mut ask,
        step: &mut step,
        rename: &mut rename,
    };
    flow::run(args, &mut world)
}

/// 问一句、读一行：空的、`y`、`yes` 是升，别的不升，标准输入关着的是没得答。
fn asked(input: &mut dyn BufRead, question: &str) -> Answer {
    let mut out = io::stdout();
    if write!(out, "{question}")
        .and_then(|()| out.flush())
        .is_err()
    {
        // 输出关了：照样读答。
    }
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(0) | Err(_) => Answer::Closed,
        Ok(_) => match answer.trim().to_lowercase().as_str() {
            "" | "y" | "yes" => Answer::Yes,
            _ => Answer::No,
        },
    }
}

/// 跑新版的 `<程序> --version`，交回它印的。
fn version_of(program: &Path) -> Result<String, String> {
    let done = std::process::Command::new(program)
        .arg("--version")
        .output()
        .map_err(|error| error.to_string())?;
    match done.status.success() {
        true => Ok(String::from_utf8_lossy(&done.stdout).into_owned()),
        false => Err(format!("{}", done.status)),
    }
}

/// 说出来：成了的在标准输出上，没成的在标准错误上。
fn say(language: Language, said: &Said) {
    let text: String = language
        .upgrade(said)
        .iter()
        .map(|line| format!("{line}\n"))
        .collect();
    let written = match said.code() {
        exit::OK => io::stdout().write_all(text.as_bytes()),
        _ => io::stderr().write_all(text.as_bytes()),
    };
    if written.is_err() {
        // 输出关了：没有别处可说。
    }
}
