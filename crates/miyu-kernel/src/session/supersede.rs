//! 撤掉重发（施工 4-15，项目主人 2026-10-14 定）：回合进行中来了一条消息，在路上的主请求一个字都还没回（连思考都没有），
//! 就撤掉这次请求，带上新消息重发。已经开始回字的照旧排着队，等下一次请求；摘要请求不撤。

use super::Session;
use super::action::Action;
use super::turn::Stage;
use crate::event::Event;
use crate::id::CommandId;
use crate::time::Timestamp;

impl Session {
    /// 人发来的一条消息排进了队（`send.rs`）：在路上的主请求一段增量都还没到的，撤掉它，记一条结果是 `superseded` 的
    /// `model.called`，`cause` 是这条消息的命令；回到「准备好」，这一批落了盘照有效历史再组装、重发，新消息就在里面。
    /// 撤掉的这一次不算步数，发之前这一轮切过的权限、环境照查一遍。返回追加的事件和叫执行器别再发了的动作；不撤的
    /// 什么都不返回。
    pub(super) fn supersede(
        &mut self,
        at: Timestamp,
        cause: &CommandId,
    ) -> Option<(Vec<Event>, Action)> {
        let turn = self.turn.as_mut()?;
        let silent = matches!(&turn.stage, Stage::Asking(call) if call.compaction.is_none() && call.silent());
        if !silent {
            return None;
        }
        let Stage::Asking(call) = std::mem::replace(&mut turn.stage, Stage::Ready) else {
            return None;
        };
        turn.retrying = true;
        let seen = call.seen;
        let mut events = self.superseded(at, call, Some(cause.clone()));
        events.extend(self.refresh_facts(at));
        Some((events, Action::CancelModel { seen }))
    }
}
