//! 会话表造 fork 子代理（施工 A-F 中，`docs/blueprint/agents.md`「fork 子代理」）：照父会话落了盘的日志，从派它的那次调用那一步
//! 分叉（`miyu_session::fork` 的 `within`），头几条落了盘再载入它。同造子会话：父会话已经不在表里的不再造，拿着表的锁写第
//! 一条，删父会话的一头照样认得出它。

use std::sync::Arc;

use miyu_kernel::id::SessionId;
use miyu_kernel::origin::{By, Session};
use miyu_session::{Fork, ForkChild, Within, new_id};

use super::{Sessions, now};
use crate::Core;

impl Sessions {
    /// 造 fork 子代理：见模块的说明。原因是英文的一句，执行器记进运行日志。
    pub(crate) async fn fork_child(
        &self,
        core: &Arc<Core>,
        child: ForkChild,
    ) -> Result<SessionId, String> {
        let ForkChild {
            lineage,
            command,
            call,
            placeholder,
        } = child;
        let at = now();
        let id = new_id(at);
        {
            let open = self.open.lock().await;
            // 父会话被删了：它停下之前在派的不再造（同 [`Sessions::spawn`]）。
            if !open.running.contains_key(&lineage.parent) {
                return Err("the parent session is gone".to_string());
            }
            let by = By::Session(Session {
                id: lineage.parent.clone(),
            });
            miyu_session::fork(Fork {
                root: &core.root,
                parent: &lineage.parent,
                id: id.clone(),
                cause: command,
                by,
                at,
                index: Some(core.index()),
                seal_after: None,
                within: Some(Within {
                    call,
                    depth: lineage.depth,
                    placeholder,
                }),
            })
            .await
            .map_err(|error| error.to_string())?;
        }
        self.get(core, &id)
            .await
            .map_err(|refusal| format!("fork {id} not opened: {refusal:?}"))?;
        core.listing.changed(core, id.clone());
        Ok(id)
    }
}
