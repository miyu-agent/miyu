//! `miyu core status`（施工 CL-8，2026-11-18 项目主人定）：核心在不在跑、哪一版、数据根在哪。只看，不拉起。
//!
//! - 在跑：连上、握手，版本照核心自己报的（`hello` 回应的 `core.version`：升级以后没重启的，和这个程序的版本不一样）。印
//!   「核心正在运行」、版本、数据根，退出码 0。
//! - 没在跑：印「核心没有在运行」、数据根，接一行提示怎么起，退出码 [`NOT_RUNNING`]（3，同 `systemctl status`；网页的
//!   `miyu web status` 照同一套，施工 CL-8 和网页那边对过）。
//! - 说不准的：连不上（核心跑着时 `run/` 被删了，施工 CS-3）、握手被拒，照设计 47 说在标准错误上，退出码 1。
//!
//! `--format json` 印一行 `{"running", "version", "data_root"}`：没在跑的 `version` 是 `null`。这一行在跑、没在跑都印在标准
//! 输出上，退出码照上面。

use std::io::Write;
use std::process::ExitCode;

use serde_json::json;

use miyu_ipc::ConnectError;
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::ask::Format;
use crate::diagnostic::{self, Problem};
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;

/// 核心没在跑时 `miyu core status` 的退出码。
pub const NOT_RUNNING: u8 = 3;

/// 跑一次 `miyu core status`，交回退出码。
pub fn core_status(format: Format) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return ExitCode::from(diagnostic::fail(error.to_string())),
    };
    let language = language::current();
    let root = match DataRoot::locate(&Env::current()) {
        Ok(root) => root,
        Err(error) => return ExitCode::from(diagnostic::fail(crate::startup::spoken(&error))),
    };
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    ExitCode::from(runtime.block_on(core_status_on(&root, format, language, &mut out, &mut err)))
}

/// 同 [`core_status`]：数据根 `root`，结果印在 `out` 上，出错照 `language` 说在 `err` 上。
pub async fn core_status_on(
    root: &DataRoot,
    format: Format,
    language: Language,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let (connection, token) = match miyu_ipc::connect(root).await {
        Ok(connected) => connected,
        Err(ConnectError::NotRunning) => {
            return match miyu_ipc::running_unreachable(root).await {
                Ok(false) => shown(out, format, language, root, None),
                Ok(true) => failed(
                    err,
                    language,
                    &crate::core_stop::unreachable(root, language),
                ),
                Err(error) => failed(err, language, &Problem::new(error.to_string())),
            };
        }
        Err(error) => {
            return failed(err, language, &Problem::new(crate::startup::spoken(&error)));
        }
    };
    let mut rpc = Rpc::new(connection, "core-status");
    let hello = match link::hello(&mut rpc, &token, &language, false, err).await {
        Ok(hello) => hello,
        Err(code) => return code,
    };
    let language = link::spoken(&hello, language);
    let version = hello["core"]["version"].as_str().unwrap_or_default();
    shown(out, format, language, root, Some(version))
}

/// 印结果，交回退出码：`version` 有的是在跑（核心报的版本），没有的是没在跑。
fn shown(
    out: &mut dyn Write,
    format: Format,
    language: Language,
    root: &DataRoot,
    version: Option<&str>,
) -> u8 {
    let data_root = root.path().display().to_string();
    let text = match format {
        Format::Json => format!(
            "{}\n",
            json!({"running": version.is_some(), "version": version, "data_root": data_root})
        ),
        Format::Text => language
            .core_status(version, &data_root)
            .iter()
            .map(|line| format!("{line}\n"))
            .collect(),
    };
    if out.write_all(text.as_bytes()).is_err() {
        // 输出关了：没有别处可说。
    }
    match version {
        Some(_) => exit::OK,
        None => NOT_RUNNING,
    }
}

/// 照设计 47 说在 `err` 上，交回退出码 1。
fn failed(err: &mut dyn Write, language: Language, problem: &Problem) -> u8 {
    diagnostic::error(err, language, problem);
    exit::ERROR
}

impl Language {
    /// `miyu core status` 给人看的几行：在跑的是状态、版本、数据根；没在跑的是状态、数据根、怎么起。
    pub(crate) fn core_status(&self, version: Option<&str>, data_root: &str) -> Vec<String> {
        match (self, version) {
            (Language::Chinese, Some(version)) => vec![
                "核心正在运行".to_string(),
                format!("版本：{version}"),
                format!("数据根：{data_root}"),
            ],
            (Language::English, Some(version)) => vec![
                "The core is running".to_string(),
                format!("version: {version}"),
                format!("data root: {data_root}"),
            ],
            (Language::Chinese, None) => vec![
                "核心未运行".to_string(),
                format!("数据根：{data_root}"),
                "提示：运行 'miyu core start' 启动".to_string(),
            ],
            (Language::English, None) => vec![
                "The core is not running".to_string(),
                format!("data root: {data_root}"),
                "hint: run 'miyu core start' to start it".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests;
