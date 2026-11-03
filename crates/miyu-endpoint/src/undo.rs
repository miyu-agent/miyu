//! 撤销、恢复的回应里给人看的几样（`docs/designs/04-核心协议.md` 第九节，施工 4-7 下；重做的撤销那一半也照它写，施工 4-7
//! 再补）：撤的是哪一轮（那一轮人说的那句话）、撤掉了几次压缩（施工 6-9）、几次清空（施工 6-8 补）、停掉了哪几个任务
//! （施工 7-8，`jobs.rs`）。「该显示什么」写在核心里（`01-架构.md` D1），头照着印。
//!
//! 施工 U-1 拿掉了每个文件怎样、差异那一格 `files` 和执行过几条命令那一格 `commands`（它只为说一句「命令改的文件撤不回」）：
//! 撤销不再改回文件（2026-10-15 项目主人定），回执也不提文件。
//!
//! 照会话的日志读：撤销（恢复）落了盘才回应，这时读得到。碰磁盘，在阻塞线程里调。
//!
//! 只读要的那几条（施工 V-41 中）：撤销那一条、撤的第一轮的开头、开它的那句话照序号各读一条（`miyu_store::log::read_one`），
//! 日志的第一条只读开头那一行；撤销的另从撤的第一轮的开头读到最后（`read_range_only`，只解要的种类），数压缩、记任务。以前整份读进内存再挑，十万条的会话撤一次多 200 MB。
//! 定位跳过的那一截不自检（2026-11-03 项目主人定，同施工 V-37 下）。

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, CompactTrigger, Event};
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_store::log::{Flow, OpenError, read_one, read_range_only};

use crate::Core;

mod jobs;

/// 撤销、恢复被接受了：回应是它产生的事件的序号 `events`、会话的工作目录 `cwd`，和给人看的几样。
pub(crate) async fn reply(core: &Core, session: &SessionId, cwd: &str, events: Vec<u64>) -> Value {
    let sources = Sources {
        log: core.root.session_dir(session),
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
            tracing::error!(target: "miyu::endpoint", panic = error.is_panic(), "undo report panicked");
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
    match written(&sources.log, events) {
        Ok(report) => report,
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "undo report not written");
            Report::default()
        }
    }
}

/// 读回应要的种类，别的只过一遍（施工 V-41 中，`Event::from_line_only`）：撤销、恢复那一条，回合的开头和开它的人话，压缩，
/// 记任务看的工具结果和两种回报。
const KINDS: &[&str] = &[
    "turn.reverted",
    "turn.unreverted",
    "turn.started",
    "message.user",
    "context.compacted",
    "tool.result",
    "job.reported",
    "child.reported",
];

/// 同 [`report`]，读不出来的交回读的错。
fn written(dir: &Path, events: &[u64]) -> Result<Report, OpenError> {
    let Some(first) = events.first().and_then(|seq| Seq::new(*seq)) else {
        return Ok(Report::default());
    };
    let Some(first) = read_one(dir, first)? else {
        return Ok(Report::default());
    };
    let (turns, undo) = match first.body {
        Body::TurnReverted(reverted) => (reverted.turns, true),
        Body::TurnUnreverted(unreverted) => (unreverted.turns, false),
        _ => return Ok(Report::default()),
    };
    // 账本不许撤、恢复空的一串（`ledger/undo.rs`）：日志里不会有，照以前交回撤了 0 轮。
    let Some(&opened) = turns.first() else {
        return Ok(Report::default());
    };
    if !undo {
        let started = read_one(dir, opened.started())?;
        return Ok(Report {
            turns: turns.len(),
            said: said(dir, started.as_ref())?,
            ..Report::default()
        });
    }
    let undone = Undone::read(dir, &turns, first.seq)?;
    let said = said(dir, undone.started.as_ref())?;
    Ok(Report {
        turns: turns.len(),
        said,
        compactions: Some(undone.compactions).filter(|count| *count > 0),
        clears: Some(undone.clears).filter(|count| *count > 0),
        jobs: undone.jobs.stopped(dir)?,
    })
}

/// 撤销从撤的第一轮的开头读到最后记下的：那一轮的开头、撤的几轮里的压缩和清空、任务。撤的几轮里的事件都不早于第一轮的
/// 开头（账本只许带着正在进行的回合），所以以前整份数的、挑的，从这里读起一样。
struct Undone {
    /// 撤的第一轮的 `turn.started`。
    started: Option<Event>,
    /// 撤的几轮里的压缩（清空不算）。
    compactions: usize,
    /// 撤的几轮里的清空。
    clears: usize,
    /// 撤销以前派出去的任务。
    jobs: jobs::Tally,
}

impl Undone {
    /// 从 `turns` 第一轮的开头读到最后；任务只记撤销那一条（第 `revert` 条）以前的。
    fn read(dir: &Path, turns: &[TurnId], revert: Seq) -> Result<Undone, OpenError> {
        let from = turns.first().map_or(Seq::FIRST, |turn| turn.started());
        let mut undone = Undone {
            started: None,
            compactions: 0,
            clears: 0,
            jobs: jobs::Tally::default(),
        };
        read_range_only(dir, from, None, KINDS, |events| {
            for event in events {
                undone.note(event, turns, revert);
            }
            Flow::Go
        })?;
        Ok(undone)
    }

    /// 记下一条。
    fn note(&mut self, event: Event, turns: &[TurnId], revert: Seq) {
        let undone = event.turn.is_some_and(|turn| turns.contains(&turn));
        match counted(&event, turns) {
            Some(true) => self.clears += 1,
            Some(false) => self.compactions += 1,
            None => {}
        }
        if event.seq < revert {
            self.jobs.note(&event, undone);
        }
        let opened = turns.first().map(|turn| turn.started());
        if Some(event.seq) == opened && matches!(event.body, Body::TurnStarted(_)) {
            self.started = Some(event);
        }
    }
}

/// 开那一轮的 `turn.started` 照触发找引起它的那一条，是人亲口说的才算，交回那句话的第一行。没有触发的（手动压缩、清空单开的
/// 那一轮，施工 6-8、6-8 补）没有。
fn said(dir: &Path, started: Option<&Event>) -> Result<Option<String>, OpenError> {
    let Some(trigger) = started.and_then(|event| match &event.body {
        Body::TurnStarted(started) => started.trigger,
        _ => None,
    }) else {
        return Ok(None);
    };
    Ok(read_one(dir, trigger)?.as_ref().and_then(first_line))
}

/// 人亲口说的那一条的第一行；不是人话、不是人亲口说的没有。
fn first_line(event: &Event) -> Option<String> {
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

/// 这一条是不是撤的几轮里的压缩（施工 6-9）：`context.compacted` 带的回合在这几轮里的。是清空（`trigger` 是 `clear`，施工 6-8
/// 补）的交回真，别的几种交回假，不是的没有。
fn counted(event: &Event, turns: &[TurnId]) -> Option<bool> {
    if !event.turn.is_some_and(|turn| turns.contains(&turn)) {
        return None;
    }
    match &event.body {
        Body::ContextCompacted(compacted) => Some(compacted.trigger == Some(CompactTrigger::Clear)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
