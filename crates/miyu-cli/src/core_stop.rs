//! `miyu core stop`（施工 CL-4，桌面端 DT-4 下要的：「退出」连核心一起关）：连已经在跑的核心（不拉起），握手、发
//! `core.stop`，再等它真的退了才回：阻塞地拿一次 `run/core.lock`，进程没了操作系统放锁，这里就醒，不轮询。
//!
//! 核心没在跑的什么都不说，退出码 0；被拒绝、连不上、等太久的照设计 47 说在标准错误上，退出码 1。成了什么都不印，同
//! `systemctl stop`。

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use serde_json::json;

use miyu_ipc::ConnectError;
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::diagnostic::{self, Problem};
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;

/// 回了以后最多等核心退多久：放下进行中的回合、写完盘一般几秒，等不到的说一声。
const WAIT: Duration = Duration::from_secs(60);

/// 跑一次 `miyu core stop`，交回退出码。
pub fn core_stop() -> ExitCode {
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
        Err(error) => return ExitCode::from(diagnostic::fail(error.to_string())),
    };
    ExitCode::from(runtime.block_on(core_stop_on(&root, language, &mut std::io::stderr())))
}

/// 同 [`core_stop`]：数据根 `root`，出错照 `language` 说在 `err` 上。
pub async fn core_stop_on(root: &DataRoot, language: Language, err: &mut dyn Write) -> u8 {
    let (connection, token) = match miyu_ipc::connect(root).await {
        Ok(connected) => connected,
        Err(ConnectError::NotRunning) => return exit::OK,
        Err(error) => return said(err, language, &Problem::new(error.to_string())),
    };
    let mut rpc = Rpc::new(connection, "core-stop");
    if let Err(code) = link::hello(&mut rpc, &token, &language, false, err).await {
        return code;
    }
    if let Err(code) = link::request(&mut rpc, "core.stop", json!({}), &language, err).await {
        return code;
    }
    drop(rpc);
    let waiting = {
        let root = root.clone();
        tokio::task::spawn_blocking(move || miyu_ipc::wait_released(&root))
    };
    match tokio::time::timeout(WAIT, waiting).await {
        Ok(Ok(Ok(()))) => exit::OK,
        Ok(Ok(Err(error))) => said(err, language, &Problem::new(error.to_string())),
        Ok(Err(error)) => said(err, language, &Problem::new(error.to_string())),
        Err(_) => said(err, language, &not_stopped(language)),
    }
}

/// 照设计 47 说在 `err` 上，交回退出码 1。
fn said(err: &mut dyn Write, language: Language, problem: &Problem) -> u8 {
    diagnostic::error(err, language, problem);
    exit::ERROR
}

/// 回了「停」、等了 [`WAIT`] 还没退。
fn not_stopped(language: Language) -> Problem {
    let seconds = WAIT.as_secs();
    match language {
        Language::Chinese => Problem::new(format!("核心在 {seconds} 秒内没有退出")),
        Language::English => {
            Problem::new(format!("the core did not exit within {seconds} seconds"))
        }
    }
}

#[cfg(test)]
mod tests;
