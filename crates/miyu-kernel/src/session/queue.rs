//! 排队的消息（`docs/designs/02-内核.md` 第六节「排队的消息」）：回合进行中来的消息排着队，
//! 下一次请求里就有它；最后一步里排着的，回合结束时接着开一轮；打断退回时撤回来。回合中途到的回报照同一条走（施工
//! 7-2，`jobs.rs`）：只是打断时不撤回、也不由它接着开，没人看着的一次性会话不由它接着开。不打断也退得回一条（施工 Q-W，
//! `session.withdraw`）。

use super::{Action, Reason, Session, rejected};
use crate::event::{Body, EndReason, Event, MessageWithdrawn};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 回合进行中来了一条消息：排进队里，等下一次请求。
    pub(super) fn enqueue(&mut self, message: Seq, cause: CommandId) {
        if let Some(turn) = self.turn.as_mut() {
            turn.queued.push((message, Some(cause)));
        }
    }

    /// 结束正在进行的回合。还有排着队的消息、没听到的回报，接着开下一轮，由最后那一条触发，`cause` 是它的；两轮在
    /// 同一批里追加。回报不由打断接着开：人刚叫停，留到下一轮；没人看着的一次性会话也不开（施工 7-2）。返回追加的事件。
    pub(super) fn finish_turn(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        reason: EndReason,
    ) -> Vec<Event> {
        let reports = reason != EndReason::Interrupted && !self.unwatched();
        let next = self.turn.as_ref().and_then(|turn| {
            let message = turn.queued.last().cloned();
            let report = turn
                .reports
                .last()
                .filter(|_| reports)
                .map(|arrived| (arrived.seq, arrived.cause.clone()));
            message
                .into_iter()
                .chain(report)
                .max_by_key(|(seq, _)| *seq)
        });
        let mut events = vec![self.end_turn(at, by, cause.clone(), reason)];
        match next {
            Some((trigger, cause)) => events.extend(self.open_turn(at, trigger, cause)),
            None => events.extend(self.seal_if_done(at, cause)),
        }
        events
    }

    /// 打断退回：把排着队的撤回来，追加一条 `message.withdrawn`，`by` 是打断的人、`cause` 是
    /// 打断的命令。没有排着队的，什么都不追加。撤回以后队里是空的，回合结束时不再接着开。
    pub(super) fn withdraw_queued(
        &mut self,
        at: Timestamp,
        by: &By,
        cause: &CommandId,
    ) -> Option<Event> {
        let turn = self.turn.as_mut()?;
        let messages: Vec<Seq> = turn.queued.drain(..).map(|(seq, _)| seq).collect();
        if messages.is_empty() {
            return None;
        }
        let body = Body::MessageWithdrawn(MessageWithdrawn { messages });
        Some(self.record(at, by.clone(), Some(cause.clone()), body))
    }

    /// 不打断，退回一条排着队的消息（施工 Q-W）：`message` 不写的退回最新的一条，追加一条只有它的 `message.withdrawn`，
    /// 回合照跑。还没被请求带上的才退得回：写了、不在队里的拒绝 `already_heard`（请求带上了，或者由它接着开了一轮）；没写、
    /// 队里是空的拒绝 `nothing_queued`。
    pub(super) fn withdraw(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        message: Option<Seq>,
    ) -> Vec<Action> {
        let queued = self.turn.as_ref().map_or(&[][..], |turn| &turn.queued[..]);
        let found = match message {
            Some(message) => queued
                .iter()
                .position(|(seq, _)| *seq == message)
                .ok_or(Reason::AlreadyHeard),
            None => queued.len().checked_sub(1).ok_or(Reason::NothingQueued),
        };
        let (Ok(index), Some(turn)) = (found, self.turn.as_mut()) else {
            return vec![rejected(id, found.err().unwrap_or(Reason::NothingQueued))];
        };
        let (seq, _) = turn.queued.remove(index);
        let body = Body::MessageWithdrawn(MessageWithdrawn {
            messages: vec![seq],
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }
}
