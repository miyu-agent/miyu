//! fork 子代理的分叉（施工 A-F 上，`docs/blueprint/agents.md`「fork 子代理」）：分叉点在父会话这一轮当中，就是派它的那一步
//! （那条带调用的 `message.assistant`）。子会话接着那一轮：头几条给那一步的每个调用记一条占位结果（几个兄弟 fork 一字不差，
//! 请求前缀才一样），再以 `forked` 收住那一轮；它自己的活由交代另开一轮。普通分叉（`session.fork`）的分叉点照旧在一轮之外。

use crate::block::{Block, Text};
use crate::event::{Body, EndReason, Event, Said, ToolResult, ToolStatus, TurnEnded};
use crate::id::{CommandId, TurnId};
use crate::ledger::Ledger;
use crate::origin::By;
use crate::time::Timestamp;

use super::super::Session;

/// 占位结果给人看的那一句（`core/tool-results/forked.txt`）。
const SAID: &str = "core/tool-results/forked";

/// 底子停在第 `turn` 轮当中、一步刚派出去的调用上：账本 `ledger` 是它过完的，`replied` 是它的最后一条是 `message.assistant`
/// （施工 V-39 中：只看最后一条，底子不用整份在手上）。交回不合的原因。
pub(super) fn check(replied: bool, ledger: &Ledger, turn: TurnId) -> Result<(), &'static str> {
    if ledger.open_turn() != Some(turn) {
        return Err("a fork within a turn stops inside that turn");
    }
    match replied && !ledger.pending_calls().is_empty() {
        true => Ok(()),
        false => Err("a fork within a turn stops right after a step that calls tools"),
    }
}

/// 子会话的账本 `ledger` 接着底子：分叉点在一轮当中（`within`）的，那一轮和那一步等结果的调用照父会话的账本 `parent`
/// （[`Ledger::continuing`]）。
pub(in crate::session) fn continued(
    ledger: Ledger,
    within: Option<TurnId>,
    parent: &Ledger,
) -> Ledger {
    match within {
        Some(_) => ledger.continuing(parent),
        None => ledger,
    }
}

impl Session {
    /// 收住接过来的第 `turn` 轮（施工 A-F 上）：那一步每个还等结果的调用照先后记一条占位结果（字照 `placeholder`，`by` 是
    /// 内核），再记 `turn.ended`（`forked`）。都带这一轮的编号，命令编号是造会话的那一个 `id`。
    pub(in crate::session) fn settle_inherited(
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
