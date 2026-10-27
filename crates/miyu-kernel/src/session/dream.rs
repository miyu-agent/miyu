//! 整理会话跑完了一趟（施工 DR-1，设计 `41-整理会话.md`）：核心以记忆模块的名义交来这一趟做了什么，记一条 `dream.ended`。
//! 不带回合编号、不进她的上下文；会话列表照它给这一趟写 `result`。

use super::Session;
use super::action::Action;
use crate::event::{Body, DreamEnded};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 记下这一趟整理的结果，落了盘回应。不查是不是整理会话：只有核心发给它自己开的整理会话。
    pub(super) fn end_dream(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        ended: DreamEnded,
    ) -> Vec<Action> {
        let event = self.record_outside(at, by, Some(id.clone()), Body::DreamEnded(ended));
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }
}
