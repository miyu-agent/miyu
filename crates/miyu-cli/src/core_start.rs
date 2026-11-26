//! `miyu core start`（施工 CL-8，2026-11-18 项目主人定：`miyu core start | status | stop` 只管核心）：核心没在跑就拉起来，等它
//! 说好了、连得上了才回；已经在跑的什么都不做。只拉核心：不开网页、不开浏览器、不开界面。
//!
//! 成了什么都不印，退出码 0，同 `miyu core stop`。起不来的（它说了原因的、没说就退了的、等太久的）照设计 47 说在标准错误上，
//! 退出码 1。人停过的（`run/stopped`，施工 CS-5）照样拉：这就是人要它起来；核心起来时自己删掉那个记号。拉起来的核心同头
//! 拉起的一样，照 `core.idle_exit` 走（出厂一直开着）。

use std::io::Write;
use std::process::{Command, ExitCode};

use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::diagnostic::{self, Problem};
use crate::exit;
use crate::language::{self, Language};
use crate::startup::connect_or_start;

/// 跑一次 `miyu core start`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn core_start(start: impl FnOnce() -> Command) -> ExitCode {
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
    ExitCode::from(runtime.block_on(core_start_on(
        &root,
        start,
        language,
        &mut std::io::stderr(),
    )))
}

/// 同 [`core_start`]：数据根 `root`，出错照 `language` 说在 `err` 上。
pub async fn core_start_on(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
    language: Language,
    err: &mut dyn Write,
) -> u8 {
    if let Err(error) = root.prepare() {
        return said(err, language, &crate::startup::spoken(&error));
    }
    match connect_or_start(root, start).await {
        // 连上了就是起来了：不握手，马上放开。
        Ok(_) => exit::OK,
        Err(error) => said(err, language, &crate::startup::spoken(&error)),
    }
}

/// 照设计 47 说在 `err` 上，交回退出码 1。
fn said(err: &mut dyn Write, language: Language, reason: &str) -> u8 {
    diagnostic::error(err, language, &Problem::new(reason));
    exit::ERROR
}
