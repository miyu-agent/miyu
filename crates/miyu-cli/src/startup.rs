//! 连上核心以前出的错照界面语言说（施工 EN-1）：找数据根、建骨架、连核心、拉起核心出的错，底层（`miyu-store`、`miyu-ipc`）
//! 拿不到界面语言，只交说法（内核的 `start/<代码>`）；这里照握手以前的界面语言（系统的语言，[`language::current`]）从资源
//! 目录的字里取。取不到的（没有说法、资源目录找不到）照底层的原话。
//!
//! 命令行的命令连核心都照这里的 [`connect_or_start`]（施工 CS-6）：慢了在标准错误上说一句。

use std::fmt::Display;
use std::process::Command;

use miyu_ipc::{ConnectError, Connection, StartError};
use miyu_kernel::event::Said;
use miyu_store::env::Env;
use miyu_store::root::{DataRoot, PrepareError, RootError};

use crate::language::{self, Language};

/// 连上核心以前出的一种错：底层交的说法。
pub(crate) trait Early: Display {
    /// 给人看的说法，没有的照原话。
    fn said(&self) -> Option<Said>;
}

impl Early for RootError {
    fn said(&self) -> Option<Said> {
        Some(RootError::said(self))
    }
}

impl Early for PrepareError {
    fn said(&self) -> Option<Said> {
        PrepareError::said(self)
    }
}

impl Early for ConnectError {
    fn said(&self) -> Option<Said> {
        ConnectError::said(self)
    }
}

impl Early for StartError {
    fn said(&self) -> Option<Said> {
        StartError::said(self)
    }
}

/// 照握手以前的界面语言说 `error`。
pub(crate) fn spoken(error: &dyn Early) -> String {
    let language = language::current();
    error
        .said()
        .and_then(|said| crate::ask::human(&Env::current(), &language).say(&said))
        .unwrap_or_else(|| error.to_string())
}

/// 连核心、没在跑就拉起来（施工 CS-6）：命令行的命令都照它连。慢了（过了 3 秒还没连上）在标准错误上说一句「核心还在启动…」
/// （[`notice`]），照握手以前的界面语言；接着等，最多 10 分钟（`miyu_ipc::Patience`）。标准输出不碰：`miyu ask --format jsonl`、
/// `miyu acp` 的标准输出是给程序读的。
///
/// # Errors
///
/// 同 `miyu_ipc::connect_or_start`。
pub(crate) async fn connect_or_start(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
) -> Result<(Connection, String), StartError> {
    let language = language::current();
    miyu_ipc::connect_or_start_waiting(root, start, &move || notice(language, Slow::Starting)).await
}

/// 同 [`connect_or_start`]，只是断线以后自己重连用（`miyu acp`，施工 ACP-9）：人停的不拉。
///
/// # Errors
///
/// 同 `miyu_ipc::reconnect_or_start`。
pub(crate) async fn reconnect_or_start(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
) -> Result<(Connection, String), StartError> {
    let language = language::current();
    miyu_ipc::reconnect_or_start_waiting(root, start, &move || notice(language, Slow::Starting))
        .await
}

/// 慢了的是哪一段（施工 CS-6）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slow {
    /// 连核心、拉起核心：「核心还在启动…」。
    Starting,
    /// 握手没回：「核心还没有回应，继续等待…」。
    Answering,
}

/// 在标准错误上说一句「慢了」（`miyu: <话>`，施工 CS-6），照 `language` 取内核 `human/` 的字；取不到的照英文。
pub(crate) fn notice(language: Language, slow: Slow) {
    let (key, english) = match slow {
        Slow::Starting => ("core/start/slow", "the core is still starting…"),
        Slow::Answering => (
            "core/start/answer_slow",
            "the core hasn't answered yet, still waiting…",
        ),
    };
    let said = crate::ask::human(&Env::current(), &language).say(&Said::new(key));
    crate::diagnostic::note(&mut std::io::stderr(), said.as_deref().unwrap_or(english));
}

/// 握手一直没回、等过了上限的那一句（施工 CS-6），照 `language`；取不到字的照英文。
pub(crate) fn no_answer(language: Language) -> String {
    crate::ask::human(&Env::current(), &language)
        .say(&Said::new("core/start/no_answer"))
        .unwrap_or_else(|| "The core never answered, stopped waiting".to_string())
}

#[cfg(test)]
mod tests;
