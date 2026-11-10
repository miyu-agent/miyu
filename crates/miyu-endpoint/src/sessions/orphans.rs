//! 派到一半的空子会话（施工 7-8，`docs/blueprint/agents.md` 第一条第 7 条，`protocol.md`「会话表」第 8 条）：派子代理时
//! 子会话造好了，交代没送进去（调用交回派不了），或者父会话没来得及记下 `job.started` 就崩了，就留下一个父会话认不得的
//! 子会话。父会话载入以后在后台收掉（施工 V-2 三补：原来在载入的路上、拿着表的锁读一遍整份日志，重开大会话要多等一遍读）：
//! 会话表里 `parent` 是它、它的日志里又没有这个子会话的 `job.started` 的，连同它们派的，停下、挪进回收处（照删会话，
//! `delete.rs`，施工 3-8 三补）。认在阻塞线程里、不拿表的锁；认出来有的才拿锁收。
//!
//! 只在父会话的日志里有没派成的 `subagent` 调用（结果里没有 `job.started`，或者还没有结果）时才去认：认要把会话表里每个会话
//! 的第一条都读一遍，平常的载入不该为它慢下来。改名以前造的会话，日志里的调用叫 `agent`，一样认（施工 7-5 再补）。
//!
//! 有没有没派成的调用、哪些子会话记下了 `job.started`，是载入读日志时一条条记下交回的（施工 V-37 上，
//! `miyu_session::Placed::recorded`）：原来这里另把父会话的整份日志读一遍，大会话每载入一次多一份整份日志在内存里。照的是
//! 载入读完日志的那一刻，比原来后台读完的那一刻早，不晚。

use std::collections::BTreeSet;
use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::id::SessionId;
use miyu_store::log::first_event;
use miyu_store::root::DataRoot;
use miyu_store::trash;

use super::delete::descendants;
use super::{Open, now};
use crate::Core;
use crate::list::forget;

/// 在后台收掉会话 `parent` 派到一半的空子会话：先不拿表的锁认，认出来有的再拿锁收（[`Open::remove_orphans`]）。`recorded`
/// 是它的日志里记下了 `job.started` 的子会话（载入时记下的）。
pub(super) fn sweep_later(core: &Arc<Core>, parent: SessionId, recorded: BTreeSet<SessionId>) {
    let core = Arc::clone(core);
    tokio::spawn(async move {
        let (root, id) = (core.root.clone(), parent.clone());
        let orphans = match tokio::task::spawn_blocking(move || orphans(&root, &id, &recorded))
            .await
        {
            Ok(orphans) => orphans,
            Err(error) => {
                tracing::error!(target: "miyu::endpoint", panic = error.is_panic(), "orphan sweep panicked");
                return;
            }
        };
        if orphans.is_empty() {
            return;
        }
        let mut open = core.sessions.open.lock().await;
        open.remove_orphans(&core, &parent, orphans).await;
    });
}

impl Open {
    /// 收掉会话 `parent` 派到一半的空子会话 `orphans`，表的锁在调的一方手里：在跑的停下（不问忙不忙，它们派的一起），目录
    /// 挪进回收处，最深的在前。挪不走的记一行运行日志。
    async fn remove_orphans(&mut self, core: &Core, parent: &SessionId, orphans: Vec<SessionId>) {
        for orphan in &orphans {
            if let Some(running) = self.running.remove(orphan)
                && running.handle.discard().await.is_err()
            {
                tracing::debug!(target: "miyu::endpoint", session = orphan.as_str(), "already stopped");
            }
        }
        self.created
            .retain(|(_, session, _)| !orphans.contains(session));
        let (root, at, parent) = (core.root.clone(), now(), parent.clone());
        let index = core.index();
        let moved = tokio::task::spawn_blocking(move || {
            for orphan in orphans {
                match trash::discard(&root, &orphan, at) {
                    Ok(()) => {
                        forget(&index, &orphan);
                        tracing::info!(target: "miyu::endpoint", session = orphan.as_str(), parent = parent.as_str(), "orphan subagent removed");
                    }
                    Err(error) => {
                        tracing::warn!(target: "miyu::endpoint", session = orphan.as_str(), error = %error, "orphan subagent not removed");
                    }
                }
            }
        })
        .await;
        if let Err(error) = moved {
            tracing::error!(target: "miyu::endpoint", panic = error.is_panic(), "orphan sweep panicked");
        }
    }
}

/// 在阻塞线程里认：`parent` 派到一半的空子会话，连同它们派的，最深的在前；`recorded` 是它的日志里记下了 `job.started` 的
/// 子会话，不算。会话列不出来的，当没有。
fn orphans(root: &DataRoot, parent: &SessionId, recorded: &BTreeSet<SessionId>) -> Vec<SessionId> {
    let Ok(ids) = root.sessions() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for id in ids.into_iter().filter(|id| !recorded.contains(id)) {
        let child_of_parent = first_event(&root.session_dir(&id)).is_ok_and(|event| {
            // 顾问会话（施工 ADV-1）不是任务，本来就没有 `job.started`，不收。
            matches!(&event.body, Body::SessionCreated(created)
                if created.parent.as_ref() == Some(parent) && !miyu_session::advising(created.origin.as_deref()))
        });
        if child_of_parent {
            let below = descendants(root, &id, false).unwrap_or_default();
            found.extend(below.into_iter().rev());
            found.push(id);
        }
    }
    found
}

#[cfg(test)]
mod tests;
