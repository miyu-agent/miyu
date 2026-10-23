//! 整理会话留多久（施工 D-1 下，设计 41 DS9）：一趟跑完，这一间只留最近几趟，多的照删会话的路删（进回收处，同人删的）；有回合
//! 在跑的不删。一间是同一个人格、或者同一个会话的记忆（`session.created` 的 `dream`）。

use std::sync::Arc;

use miyu_kernel::event::Dream;
use miyu_kernel::id::SessionId;
use miyu_tool::Stop;

use crate::Core;
use crate::list::{Listed, scan};

/// 照这一趟 `session` 的那一间，留最近 `keep` 趟（它也算），删掉别的：交回删了几个。找不到它、读不了列表、删不成的记一行运行
/// 日志。
pub(super) async fn prune(core: &Arc<Core>, session: &SessionId, keep: usize) -> usize {
    let Some(owner) = core.sessions.owner(core, session).await else {
        tracing::warn!(target: "miyu::endpoint", session = session.as_str(), "dream session not found to prune");
        return 0;
    };
    let busy = core.sessions.busy_ids().await;
    let (root, index) = (core.root.clone(), core.index_for(&owner));
    let listed = tokio::task::spawn_blocking(move || {
        let dreams = |row: &miyu_store::index::Row| row.dream.is_some();
        scan(
            &root,
            &owner,
            Some(&*index),
            &busy,
            dreams,
            None,
            &Stop::default(),
        )
    })
    .await;
    let listed = match listed {
        Ok(Ok(listed)) => listed,
        Ok(Err(error)) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "dream sessions not listed");
            return 0;
        }
        Err(error) => {
            tracing::error!(target: "miyu::endpoint", panic = error.is_panic(), "dream listing panicked");
            return 0;
        }
    };
    let mut deleted = 0;
    for id in doomed(&listed, session, keep) {
        match core.sessions.delete(core, &id).await {
            Ok(()) => deleted += 1,
            Err(refusal) => {
                tracing::warn!(target: "miyu::endpoint", session = id.as_str(), refusal = ?refusal, "old dream session not deleted");
            }
        }
    }
    deleted
}

/// 列出来的整理会话 `listed`（从新到旧）里，和 `session` 同一间的留最新的 `keep` 个，别的里没在忙的。`session` 不在里面的
/// 一个都不删。
fn doomed(listed: &[Listed], session: &SessionId, keep: usize) -> Vec<SessionId> {
    let room = |dream: &Dream| (dream.persona.clone(), dream.session.clone());
    let Some(this) = listed
        .iter()
        .find(|one| one.id == *session)
        .and_then(|one| one.dream.as_ref())
        .map(room)
    else {
        return Vec::new();
    };
    listed
        .iter()
        .filter(|one| one.dream.as_ref().map(room).as_ref() == Some(&this))
        .skip(keep)
        .filter(|one| !one.busy)
        .map(|one| one.id.clone())
        .collect()
}

#[cfg(test)]
mod tests;
