//! 撤销、恢复的回应里给人看的几样（`docs/designs/04-核心协议.md` 第九节，施工 4-7 下；重做的撤销那一半也照它写，施工 4-7
//! 再补）：撤的是哪一轮（那一轮人说的那句话）、撤掉了几次压缩（施工 6-9）、几次清空（施工 6-8 补）、停掉了哪几个任务
//! （施工 7-8，`jobs.rs`）。「该显示什么」写在核心里（`01-架构.md` D1），头照着印。
//!
//! 施工 U-1 拿掉了每个文件怎样、差异那一格 `files` 和执行过几条命令那一格 `commands`（它只为说一句「命令改的文件撤不回」）：
//! 撤销不再改回文件（2026-10-15 项目主人定），回执也不提文件。
//!
//! 照会话的日志读：撤销（恢复）落了盘才回应，这时读得到。碰磁盘，在阻塞线程里调。

use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, CompactTrigger, Event};
use miyu_kernel::id::{SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_store::log::read_events;

use crate::Core;

mod jobs;

/// 撤销、恢复被接受了：回应是它产生的事件的序号 `events`、会话的工作目录 `cwd`，和给人看的几样。
pub(crate) async fn reply(core: &Core, session: &SessionId, cwd: &str, events: Vec<u64>) -> Value {
    // 照属主的家目录读日志和 blob（施工 O-4 上）；回应时会话还在表里，找不到的照管理员（不会走到）。
    let owner = core
        .sessions
        .owner(core, session)
        .await
        .unwrap_or_else(|| core.admin.clone());
    let sources = Sources {
        log: core.root.session_dir(&owner, session),
    };
    let seqs = events.clone();
    let cwd = cwd.to_string();
    let written = tokio::task::spawn_blocking(move || {
        // 工作目录照真实的位置写：效果里的路径是真实的位置，头照它写相对的才对得上。
        let cwd =
            std::fs::canonicalize(&cwd).map_or(cwd, |real| real.to_string_lossy().into_owned());
        (report(&sources, &seqs), plain(&cwd))
    })
    .await;
    let (report, cwd) = match written {
        Ok(written) => written,
        Err(error) => {
            tracing::error!(target: "miyu::endpoint", error = %error, "undo report panicked");
            (Report::default(), String::new())
        }
    };
    let mut reply = serde_json::to_value(report).unwrap_or_else(|_| json!({}));
    reply["events"] = json!(events);
    reply["cwd"] = json!(cwd);
    reply
}

/// 给人看的路径：Windows 上真实的位置带着 `\\?\` 这个前缀，去掉；`\\?\UNC\` 的照原样。
fn plain(path: &str) -> String {
    match path.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => rest.to_string(),
        _ => path.to_string(),
    }
}

/// 写回应要照的：会话的日志在哪。
pub(crate) struct Sources {
    pub(crate) log: PathBuf,
}

/// 回应里给人看的几样。
#[derive(Debug, Default, Serialize)]
pub(crate) struct Report {
    /// 撤了（恢复了）几轮。
    turns: usize,
    /// 第一轮里人说的那句话的第一行；那一轮不是人开的，没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    said: Option<String>,
    /// 撤掉了几次压缩（施工 6-9）：撤销时才有，是 0 的不写。头照它说一句上下文回到了压缩前。清空不算在里面。
    #[serde(skip_serializing_if = "Option::is_none")]
    compactions: Option<usize>,
    /// 撤掉了几次清空（施工 6-8 补）：撤销时才有，是 0 的不写。头照它说一句上下文回到了清空以前。
    #[serde(skip_serializing_if = "Option::is_none")]
    clears: Option<usize>,
    /// 停掉的任务（施工 7-8）：撤掉的几轮派出去、撤销那一刻还在跑的，照编号。撤销时才有，没有的不写。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    jobs: Vec<jobs::Stopped>,
}

/// 照撤销（恢复）那一条（`events` 里第一个序号）写成回应里的几样。日志读不出来的，
/// 记一条运行日志，交回空的：撤销本身已经成了。
pub(crate) fn report(sources: &Sources, events: &[u64]) -> Report {
    let log = match read_events(&sources.log) {
        Ok(log) => log,
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "undo report not written");
            return Report::default();
        }
    };
    let find = |seq: u64| log.iter().find(|event| event.seq.get() == seq);
    let Some(first) = events.first().and_then(|seq| find(*seq)) else {
        return Report::default();
    };
    let (turns, undo) = match &first.body {
        Body::TurnReverted(reverted) => (reverted.turns.clone(), true),
        Body::TurnUnreverted(unreverted) => (unreverted.turns.clone(), false),
        _ => return Report::default(),
    };
    Report {
        turns: turns.len(),
        said: turns.first().and_then(|turn| said(&log, *turn)),
        compactions: undo
            .then(|| compactions(&log, &turns, false))
            .filter(|count| *count > 0),
        clears: undo
            .then(|| compactions(&log, &turns, true))
            .filter(|count| *count > 0),
        jobs: match undo {
            true => jobs::stopped(&log, first.seq, &turns),
            false => Vec::new(),
        },
    }
}

/// 回合 `turn` 里人说的那句话的第一行：照 `turn.started` 找引起它的那一条，是人亲口说的才算。没有触发的（手动压缩、
/// 清空单开的那一轮，施工 6-8、6-8 补）没有。
fn said(log: &[Event], turn: TurnId) -> Option<String> {
    let trigger = log.iter().find_map(|event| match &event.body {
        Body::TurnStarted(started) if event.seq == turn.started() => started.trigger,
        _ => None,
    })?;
    let event = log.iter().find(|event| event.seq == trigger)?;
    let (Body::MessageUser(message), By::Person(_)) = (&event.body, &event.by) else {
        return None;
    };
    // 斜杠命令模板展开成的照人敲的那一行（施工 X-14 上）。
    if let Some(typed) = message.typed() {
        return Some(typed.to_string());
    }
    message.blocks.iter().find_map(|block| match block {
        Block::Text(text) => text
            .text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_string),
        _ => None,
    })
}

/// 这几轮里有几次压缩（施工 6-9）：`context.compacted` 带的回合在这几轮里的。`clears` 是数清空的（`trigger` 是
/// `clear`，施工 6-8 补），不是的数别的几种。
fn compactions(log: &[Event], turns: &[TurnId], clears: bool) -> usize {
    log.iter()
        .filter(|event| event.turn.is_some_and(|turn| turns.contains(&turn)))
        .filter(|event| {
            matches!(&event.body, Body::ContextCompacted(compacted)
            if (compacted.trigger == Some(CompactTrigger::Clear)) == clears)
        })
        .count()
}

#[cfg(test)]
mod tests;
