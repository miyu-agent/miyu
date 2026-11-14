//! `miyu core stop`（施工 CL-4，桌面端 DT-4 下要的：「退出」连核心一起关）：连已经在跑的核心（不拉起），握手、发
//! `core.stop`，再等它真的退了才回：阻塞地拿一次 `run/core.lock`，进程没了操作系统放锁，这里就醒，不轮询。
//!
//! 核心没在跑的什么都不说，退出码 0；被拒绝、连不上、等太久的照设计 47 说在标准错误上，退出码 1。成了什么都不印，同
//! `systemctl stop`。发了 `core.stop` 以后连接断了、没等到回应的，当核心接了（它收到就停，回应不一定送得到）。
//!
//! 「没在跑」要先确认（施工 CS-3）：照 `run/socket` 连不上时再看锁和算出来的位置（`miyu_ipc::running_unreachable`）。核心跑着时
//! `run/` 被删了，令牌、位置都没了，它还开着：原来当它没在跑、退 0，现在说连不上它、怎么收掉，退 1。不另想办法结束它。

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
        Err(ConnectError::NotRunning) => {
            return match miyu_ipc::running_unreachable(root).await {
                Ok(false) => exit::OK,
                Ok(true) => said(err, language, &unreachable(root, language)),
                Err(error) => said(err, language, &Problem::new(error.to_string())),
            };
        }
        Err(error) => return said(err, language, &Problem::new(error.to_string())),
    };
    let mut rpc = Rpc::new(connection, "core-stop");
    if let Err(code) = link::hello(&mut rpc, &token, &language, false, err).await {
        return code;
    }
    // 核心收到就停：回应可能还没送到连接就断了（CL-4 修，CI 上偶发）。断了的当它接了，照样等锁；只有明说拒绝的才报。
    match rpc.call("core.stop", json!({})).await {
        Ok(Some(reply)) if reply.get("error").is_some() => {
            let message = reply["error"]["message"].as_str().unwrap_or_default();
            return said(err, language, &language.refused(message));
        }
        Ok(_) => {}
        Err(error) => return said(err, language, &Problem::new(error.to_string())),
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

/// 核心还在跑，照 `run/` 里的连不上它（施工 CS-3）：令牌、套接字的位置不见了。指给人进程号在哪：核心起来那一行记着。
fn unreachable(root: &DataRoot, language: Language) -> Problem {
    let log = root.core_log();
    let log = log.display();
    match language {
        Language::Chinese => Problem::new("核心还在跑，但连不上它：数据根 run/ 里的令牌或套接字位置不见了")
            .hint(format!("照进程号结束它：{log} 里最后一行 starting 的 pid")),
        Language::English => Problem::new(
            "the core is still running but cannot be reached: the token or socket location in the data root's run/ is gone",
        )
        .hint(format!(
            "end it by process id: the pid on the last starting line of {log}"
        )),
    }
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
