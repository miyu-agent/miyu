//! 撤回人敲的 `/loop`（施工 FX-15，主会话 2026-11-22 定，`internal/blueprint/schedules.md` 第五节第 9 条）：人撤回了自己敲的
//! 命令，循环也该停，不然撤了命令循环还接着到点跑。撤销记下以后照撤掉的几轮的触发认出人敲 `/loop` 当场开的那一轮（`by` 是
//! 人建的定时、带人敲的那一行），删掉它的那一行，整行记在内存里；恢复这一次撤销时照原样放回（编号、下一次都不变）。
//!
//! 一个会话照撤销的先后一层一层记，每次撤销一层（没认出循环的是空的一层），恢复拿走最上面那一层：内核恢复的也是最近一次。
//! 撤销以后又开了一轮、压缩过的，内核那边恢复不了，这边那几层就不会再拿走，放着不碍事。只在内存里：核心重启过的记不得，
//! 恢复不会重建循环，要再敲一次。

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use miyu_kernel::event::{Body, said_by_the_person};
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::origin::By;
use miyu_store::log::read_one;
use miyu_store::schedules::{Schedule, Target};

use super::{Schedules, TARGET};

/// 会话 → 照撤销的先后，每一次撤销删掉的那几行。
#[derive(Default)]
pub(super) struct Undone(Mutex<BTreeMap<SessionId, Vec<Vec<Schedule>>>>);

impl Undone {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<SessionId, Vec<Vec<Schedule>>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Schedules {
    /// 会话 `session` 的撤销记下了（`events` 是撤销那个命令追加的序号）：撤掉的几轮里有人敲 `/loop` 当场开的，删掉它的那一行、
    /// 记下来等恢复。日志读不出的记一行运行日志，照没有办。
    pub(crate) async fn reverted(&self, session: &SessionId, events: &[u64]) {
        let dir = self.root.session_dir(session);
        let first = events.first().and_then(|seq| Seq::new(*seq));
        let found =
            tokio::task::spawn_blocking(move || first.map(|first| typed_loops(&dir, first)))
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
        let mut taken = Vec::new();
        for id in found {
            let Ok(row) = self.get(&id) else {
                continue;
            };
            let here = matches!(&row.target, Target::Session { session: on } if on == session);
            if here && self.cancel(&id).is_ok() {
                taken.push(row);
            }
        }
        self.undone
            .lock()
            .entry(session.clone())
            .or_default()
            .push(taken);
    }

    /// 要恢复会话 `session` 的撤销：先拿走最近一次撤销删掉的那几行（恢复成了再放回，没成的交回 [`Schedules::unrevert_refused`]）。
    pub(crate) fn unreverting(&self, session: &SessionId) -> Option<Vec<Schedule>> {
        let mut undone = self.undone.lock();
        let taken = undone.get_mut(session)?.pop();
        if undone.get(session).is_some_and(Vec::is_empty) {
            undone.remove(session);
        }
        taken
    }

    /// 恢复被拒了：拿走的那一层放回原处。
    pub(crate) fn unrevert_refused(&self, session: &SessionId, taken: Option<Vec<Schedule>>) {
        if let Some(taken) = taken {
            self.undone
                .lock()
                .entry(session.clone())
                .or_default()
                .push(taken);
        }
    }

    /// 恢复成了：那几行照原样放回。放不回的（表用不了、同编号的又有了）记一行运行日志。
    pub(crate) fn unreverted(&self, taken: Option<Vec<Schedule>>) {
        for row in taken.unwrap_or_default() {
            match self.table(false).map(|table| table.restore(&row)) {
                Ok(Ok(true)) => {
                    tracing::info!(target: TARGET, schedule = %row.id, "loop put back after restore");
                    self.changed();
                    self.notify(&row.id);
                }
                _ => {
                    tracing::warn!(target: TARGET, schedule = %row.id, "loop not put back after restore");
                }
            }
        }
    }
}

/// 撤销那一条（第 `first` 条）撤掉的几轮里，由人敲 `/loop` 当场开的那几轮的任务编号。
fn typed_loops(dir: &Path, first: Seq) -> Vec<String> {
    let read = || -> Result<Vec<String>, miyu_store::log::OpenError> {
        let Some(Body::TurnReverted(reverted)) = read_one(dir, first)?.map(|event| event.body)
        else {
            return Ok(Vec::new());
        };
        let mut found = Vec::new();
        for turn in &reverted.turns {
            let trigger = match read_one(dir, turn.started())?.map(|event| event.body) {
                Some(Body::TurnStarted(started)) => started.trigger,
                _ => None,
            };
            let Some(said) = trigger
                .map(|trigger| read_one(dir, trigger))
                .transpose()?
                .flatten()
            else {
                continue;
            };
            if let (Body::MessageUser(message), By::Schedule(schedule)) = (&said.body, &said.by)
                && said_by_the_person(&said)
                && message.command.as_ref().is_some_and(super::typed::is_loop)
            {
                found.push(schedule.schedule.clone());
            }
        }
        Ok(found)
    };
    read().unwrap_or_else(|error| {
        tracing::warn!(target: TARGET, error = %error, "reverted loops not read");
        Vec::new()
    })
}
