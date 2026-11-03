//! 撤销的回应里停掉的任务（施工 7-8，`docs/blueprint/protocol/undo.md`「jobs」，`agents.md` 第七条第 1 条）：撤掉的那几轮
//! 派出去、撤销那一刻还在跑的，就是内核交给执行器停的那几个（`kernel/history.md`「撤销」）。回应不等它们的回报落盘，所以
//! 照日志算撤销那一刻的：派它的调用在撤掉的那几轮里、照撤销以前的日志还在跑（`running_jobs`）的。
//!
//! 只记撤的第一轮开头以后的（施工 V-41 中）：撤掉的那几轮里派的任务都在那以后，一个任务的样子只随它派出去以后的事件变（编号
//! 整份日志里不重复），内核的 [`JobTally`] 照账本的记法记，从那里记起和从第一条记起一样。以前整份过一遍账本，过不了的交回
//! 空的；现在账本只过日志的第一条（分叉出来的会话的第一条不是第 1 条，过不了，同以前交回空的），后面的不过：会话载入、追加
//! 时每一条都过过同一套账本。

use std::path::Path;

use serde::Serialize;

use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::JobId;
use miyu_kernel::ledger::{JobTally, Ledger};
use miyu_store::log::{OpenError, first_event};

/// 停掉的一个任务：编号、种类、标题，照 `job.started` 写。
#[derive(Debug, Serialize)]
pub(crate) struct Stopped {
    job: String,
    what: String,
    title: String,
}

/// 边读边记的：撤销以前派出去的任务的样子，和撤掉的那几轮里派出去的，照先后。
#[derive(Default)]
pub(super) struct Tally {
    /// 撤的第一轮开头以后派出去的任务的样子。
    jobs: JobTally,
    /// 撤掉的那几轮里派出去的。
    started: Vec<(JobId, Stopped)>,
}

impl Tally {
    /// 记下撤销以前的一条；`undone` 是它在撤掉的那几轮里。
    pub(super) fn note(&mut self, event: &Event, undone: bool) {
        self.jobs.record(event);
        let Body::ToolResult(result) = &event.body else {
            return;
        };
        if !undone {
            return;
        }
        for effect in &result.effects {
            if let Effect::JobStarted(started) = effect {
                self.started.push((
                    started.job.clone(),
                    Stopped {
                        job: started.job.to_string(),
                        what: started.what.as_str().to_string(),
                        title: started.title.clone(),
                    },
                ));
            }
        }
    }

    /// 撤销停掉的：撤掉的那几轮里派出去、撤销那一刻还在跑的，照编号。日志 `dir` 的第一条过不了账本的（分叉出来的会话），交回
    /// 空的：撤销本身已经成了。
    pub(super) fn stopped(self, dir: &Path) -> Result<Vec<Stopped>, OpenError> {
        let first = first_event(dir)?;
        if let Err(error) = Ledger::default().append(&first) {
            tracing::warn!(target: "miyu::endpoint", seq = error.seq.get(), "undo report jobs not read");
            return Ok(Vec::new());
        }
        let running = self.jobs.running();
        let mut stopped: Vec<(JobId, Stopped)> = self
            .started
            .into_iter()
            .filter(|(job, _)| running.contains(job))
            .collect();
        stopped.sort_by(|(one, _), (other, _)| one.cmp(other));
        Ok(stopped.into_iter().map(|(_, stopped)| stopped).collect())
    }
}
