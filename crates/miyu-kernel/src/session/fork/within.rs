//! fork 子代理的分叉（施工 A-F 上，`docs/blueprint/agents.md`「fork 子代理」）：分叉点在父会话这一轮当中，就是派它的那一步
//! （那条带调用的 `message.assistant`）。子会话接着那一轮：头几条给那一步的每个调用记一条占位结果（几个兄弟 fork 一字不差，
//! 请求前缀才一样），再以 `forked` 收住那一轮；它自己的活由交代另开一轮。普通分叉（`session.fork`）的分叉点照旧在一轮之外。

use crate::block::{Block, Text};
use crate::event::{Body, EndReason, Event, Said, ToolResult, ToolStatus, TurnEnded};
use crate::id::{CallId, CommandId, SessionId, TurnId};
use crate::ledger::Ledger;
use crate::origin::By;
use crate::time::Timestamp;

use super::super::Session;
use super::super::load::LoadError;
use super::{Base, ForkPlan, plan_upto};
use crate::ledger::LedgerError;

/// 占位结果给人看的那一句（`core/tool-results/forked.txt`）。
const SAID: &str = "core/tool-results/forked";

/// 底子 `base` 停在第 `turn` 轮当中、一步刚派出去的调用上：账本 `ledger` 是它过完的。交回不合的原因。
pub(super) fn check(base: &[Event], ledger: &Ledger, turn: TurnId) -> Result<(), &'static str> {
    if ledger.open_turn() != Some(turn) {
        return Err("a fork within a turn stops inside that turn");
    }
    match base.last().map(|event| &event.body) {
        Some(Body::MessageAssistant(_)) if !ledger.pending_calls().is_empty() => Ok(()),
        _ => Err("a fork within a turn stops right after a step that calls tools"),
    }
}

/// 子会话的账本 `ledger` 接着底子：分叉点在一轮当中的，那一轮和那一步等结果的调用照父会话的（[`Ledger::continuing`]）。
pub(in crate::session) fn continued(ledger: Ledger, base: &Base<'_>) -> Ledger {
    match base.within {
        Some(_) => ledger.continuing(&base.ledger),
        None => ledger,
    }
}

/// 从父会话 `parent` 这一轮派调用 `call` 的那一步分叉要照的几样（施工 A-F 上）：分叉点是那一步的 `message.assistant`，权限、
/// 模型、快照照到那一条为止的。只照落了盘的日志 `events` 算。
///
/// # Errors
///
/// 日志过不了账本、是空的：[`LoadError`]；`call` 不是正在进行的那一轮里等结果的调用：[`LoadError::Broken`]。
pub fn fork_plan_at(
    parent: &SessionId,
    events: &[Event],
    call: CallId,
) -> Result<ForkPlan, LoadError> {
    let mut ledger = Ledger::for_session(parent.clone());
    for event in events {
        ledger.append(event).map_err(LoadError::Broken)?;
    }
    let last = events.last().ok_or(LoadError::Empty)?.seq;
    let not_waiting = || {
        LoadError::Broken(LedgerError {
            seq: last,
            why: format!("{call} is not a call waiting for a result in the running turn"),
        })
    };
    let turn = ledger.open_turn().ok_or_else(not_waiting)?;
    if !ledger.pending_calls().contains(&call) {
        return Err(not_waiting());
    }
    let upto = ledger.last_reply().ok_or_else(not_waiting)?;
    plan_upto(events, upto, Some(turn))
}

impl Session {
    /// 收住接过来的第 `turn` 轮（施工 A-F 上）：那一步每个还等结果的调用照先后记一条占位结果（字照 `placeholder`，`by` 是
    /// 内核），再记 `turn.ended`（`forked`）。都带这一轮的编号，命令编号是造会话的那一个 `id`。
    pub(super) fn settle_inherited(
        &mut self,
        at: Timestamp,
        id: &CommandId,
        turn: TurnId,
        placeholder: &str,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        for call_id in self.ledger.pending_calls() {
            let result = ToolResult {
                call_id,
                status: ToolStatus::Ok,
                blocks: vec![Block::Text(Text {
                    text: placeholder.to_string(),
                })],
                duration_ms: None,
                human: Some(Said::new(SAID)),
                effects: Vec::new(),
                end_turn: false,
                sent: Vec::new(),
                display_name: Default::default(),
                detail: None,
            };
            events.push(self.stamped_in(at, turn, id, Body::ToolResult(result)));
        }
        let ended = TurnEnded {
            reason: EndReason::Forked,
        };
        events.push(self.stamped_in(at, turn, id, Body::TurnEnded(ended)));
        events
    }

    /// 内核记一条带第 `turn` 轮编号的事件（那一轮是父会话开的，执行器这里没有它的状态）。
    fn stamped_in(&mut self, at: Timestamp, turn: TurnId, id: &CommandId, body: Body) -> Event {
        let seq = self.ledger.next_seq();
        self.stamped(seq, at, Some(turn), (By::Kernel, Some(id.clone())), body)
    }
}
