//! 以前的撤销回应（施工 V-41 中以前的 `undo.rs`、`undo/jobs.rs`，原样抄来当准绳）：整份读进内存，照整份挑；停掉的任务照撤销
//! 以前的整份日志过一遍账本。只改了入口拿日志目录、类型留在这里（回应照 JSON 比）。

use std::path::Path;

use serde::Serialize;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, CompactTrigger, Effect, Event};
use miyu_kernel::id::{JobId, Seq, TurnId};
use miyu_kernel::ledger::Ledger;
use miyu_kernel::origin::By;
use miyu_store::log::read_events;

/// 回应里给人看的几样。
#[derive(Debug, Default, Serialize)]
pub(super) struct Report {
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
    jobs: Vec<Stopped>,
}

/// 照撤销（恢复）那一条（`events` 里第一个序号）写成回应里的几样。日志读不出来的，
/// 记一条运行日志，交回空的：撤销本身已经成了。
pub(super) fn report(dir: &Path, events: &[u64]) -> Report {
    let log = match read_events(dir) {
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
            true => stopped(&log, first.seq, &turns),
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

/// 停掉的一个任务：编号、种类、标题，照 `job.started` 写。
#[derive(Debug, Serialize)]
pub(super) struct Stopped {
    job: String,
    what: String,
    title: String,
}

/// 撤销（第 `revert` 条）停掉的：撤掉的那几轮 `turns` 里派出去、撤销那一刻还在跑的，照编号。撤销以前的日志过不了账本的
/// （坏了），交回空的：撤销本身已经成了。
fn stopped(log: &[Event], revert: Seq, turns: &[TurnId]) -> Vec<Stopped> {
    let mut ledger = Ledger::default();
    let before = log.iter().take_while(|event| event.seq < revert);
    for event in before.clone() {
        if ledger.append(event).is_err() {
            tracing::warn!(target: "miyu::endpoint", seq = event.seq.get(), "undo report jobs not read");
            return Vec::new();
        }
    }
    let running = ledger.running_jobs();
    let mut stopped: Vec<(JobId, Stopped)> = before
        .filter(|event| event.turn.is_some_and(|turn| turns.contains(&turn)))
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.effects.iter()),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            Effect::JobStarted(started) if running.contains(&started.job) => Some((
                started.job.clone(),
                Stopped {
                    job: started.job.to_string(),
                    what: started.what.as_str().to_string(),
                    title: started.title.clone(),
                },
            )),
            _ => None,
        })
        .collect();
    stopped.sort_by(|(one, _), (other, _)| one.cmp(other));
    stopped.into_iter().map(|(_, stopped)| stopped).collect()
}
