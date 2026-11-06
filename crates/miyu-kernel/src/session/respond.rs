//! 照记下的几条开一轮（施工 O-14 上，`docs/blueprint/chat.md` 第七条第 3 条第 1 项）：`Respond` 拿已经旁听记下的几条
//! `message.user` 当触发开一轮，不新记消息；带来的几块事实接在内核的事实后面，都排在触发前面。没有 `to` 的照事实开一轮
//! （施工 O-64，通讯平台到点看一眼私聊要的）：事实先记下，最后一块当触发。
//!
//! 带了 `tail` 的（施工 RT-1）：开出来的这一轮每次请求都把它接在最末尾，记在 `turn.started` 上；并进正在跑的一轮的换掉那一轮的、
//! 记在 `turn.joined` 上，没带的不动原来的。

use super::action::{Action, Reason};
use super::jobs::{Arrived, Waker};
use super::{Session, rejected, rejected_about};
use crate::event::{Body, ContextInjected, Event, TurnJoined};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 照 `to` 那几条开一轮，`facts` 原样记成 `by` 注入的事实，接在内核的事实后面。正在跑一轮的并进去（[`Session::join`]，施工
    /// O-14 下）；一条都没有的拒，`empty_message`；有不是旁听的拒，`not_ambient`；有当过触发的拒，`already_answered`：
    /// 后两种带上是哪几条。`to` 照序号排好、去重。`to` 是空的照 `facts` 开（[`Session::prompted`]，施工 O-64），两样都空的
    /// 拒，`empty_message`。
    pub(super) fn respond(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        mut to: Vec<Seq>,
        (facts, tail): (Vec<ContextInjected>, Option<String>),
    ) -> Vec<Action> {
        to.sort_unstable();
        to.dedup();
        let Some(&trigger) = to.last() else {
            return self.prompted(id, by, at, facts, tail);
        };
        let not: Vec<Seq> = to
            .iter()
            .copied()
            .filter(|seq| !self.ledger.overheard(*seq))
            .collect();
        if !not.is_empty() {
            return vec![rejected_about(id, Reason::NotAmbient, not)];
        }
        let answered: Vec<Seq> = to
            .iter()
            .copied()
            .filter(|seq| self.ledger.answered(*seq))
            .collect();
        if !answered.is_empty() {
            return vec![rejected_about(id, Reason::AlreadyAnswered, answered)];
        }
        if self.turn.is_some() {
            return self.join(id, by, at, to, (facts, tail));
        }
        let mut events = self.open_turn_on(at, trigger, to, Some(id.clone()), tail);
        for fact in facts {
            events.push(self.record(
                at,
                by.clone(),
                Some(id.clone()),
                Body::ContextInjected(fact),
            ));
        }
        if let (Some(turn), Some(last)) = (self.turn.as_mut(), events.last()) {
            turn.stage = super::turn::Stage::Opening { opened: last.seq };
        }
        self.accept(id, events.iter().map(|event| event.seq).collect());
        vec![Action::Append(events)]
    }

    /// 没有 `to`、照几块事实开一轮（施工 O-64）：一块都没有的拒，`empty_message`。正在跑一轮的照样并进去（`turn.joined` 的
    /// `triggers` 是空的）；空闲的先记下事实（不带回合编号），由最后一块开一轮，同后台命令的回报开的回合。
    fn prompted(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        facts: Vec<ContextInjected>,
        tail: Option<String>,
    ) -> Vec<Action> {
        if facts.is_empty() {
            return vec![rejected(id, Reason::EmptyMessage)];
        }
        if self.turn.is_some() {
            return self.join(id, by, at, Vec::new(), (facts, tail));
        }
        let mut events: Vec<Event> = facts
            .into_iter()
            .map(|fact| {
                self.record(
                    at,
                    by.clone(),
                    Some(id.clone()),
                    Body::ContextInjected(fact),
                )
            })
            .collect();
        let Some(trigger) = events.last().map(|fact| fact.seq) else {
            return vec![rejected(id, Reason::EmptyMessage)];
        };
        events.extend(self.open_turn_on(at, trigger, Vec::new(), Some(id.clone()), tail));
        self.accept(id, events.iter().map(|event| event.seq).collect());
        vec![Action::Append(events)]
    }

    /// 正在跑一轮时（施工 O-14 下）：桥的事实、`turn.joined` 带这个回合当场记下，照回报排进队（下一步听到；没再请求就结束的
    /// 接着开一轮，由它触发；打断时不退回、不接着开）；这一步里在等人的调用作废，同回合中途来的人的话。叫停的 `CancelTool`
    /// 排在 `Append` 后面。
    fn join(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        triggers: Vec<Seq>,
        (facts, tail): (Vec<ContextInjected>, Option<String>),
    ) -> Vec<Action> {
        let cause = Some(id.clone());
        let mut events: Vec<Event> = facts
            .into_iter()
            .map(|fact| self.record(at, by.clone(), cause.clone(), Body::ContextInjected(fact)))
            .collect();
        let joined = self.record(
            at,
            By::Kernel,
            cause.clone(),
            Body::TurnJoined(TurnJoined {
                triggers,
                tail: tail.clone(),
            }),
        );
        if let Some(turn) = self.turn.as_mut() {
            // 带了的换掉这一轮的；没带的不动：并进同一轮的消息不该把提示抹掉（core1 11-07 定）。
            if tail.is_some() {
                turn.tail = tail;
            }
            turn.reports.push(Arrived {
                seq: joined.seq,
                cause,
                waker: Waker::Joined,
            });
        }
        events.push(joined);
        let (voided, stops) = self.void_waiting(at, &by, &id);
        events.extend(voided);
        self.accept(id, events.iter().map(|event| event.seq).collect());
        let mut actions = vec![Action::Append(events)];
        actions.extend(stops);
        actions
    }
}
