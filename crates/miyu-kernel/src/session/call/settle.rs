//! 一次请求有了结局（施工 4-15 从 `call.rs` 挪出来，那边放不下了）：该写的回复写上，记一条 `model.called`。说完了、
//! 出错的由 `call.rs` 收，被打断的由 `interrupt.rs` 收，撤掉重发的由 `supersede.rs` 收。

use super::Call;
use crate::block::{Block, ToolCall};
use crate::event::{
    Body, CallError, CallResult, Cost, ErrorClass, Event, FirstDifference, MessageAssistant,
    ModelCalled, Usage,
};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::session::Session;
use crate::session::aside::bad_stream;
use crate::session::compaction::Compacting;
use crate::session::spans::millis;
use crate::session::summary::{Summarized, called_tool};
use crate::time::Timestamp;

/// 一次请求的结局。
pub(super) enum Ending {
    /// 执行器报说完了：正常说完的带用量（和金额，装在盒子里：照 `model.called` 的那一格），出错的带分类和原话。
    Said {
        usage: Option<Usage>,
        cost: Option<Box<Cost>>,
        error: Option<CallError>,
    },
    /// 被人打断。
    CutOff,
    /// 排队的消息到了、一个字都还没回，撤掉重发（施工 4-15）。
    Superseded,
}

/// 一次请求收拾完：追加的事件、写成的回复是第几条、回复里的调用、出错的分类和原话；摘要请求取到的摘要和它替代到
/// 哪。
pub(super) struct Settled {
    pub(super) events: Vec<Event>,
    pub(super) reply: Option<Seq>,
    pub(super) calls: Vec<ToolCall>,
    pub(super) error: Option<CallError>,
    pub(super) summary: Option<Summarized>,
    /// 摘要请求多记的，交回来：报了超长的照它截短再发（施工 6-6 中）。
    pub(super) compaction: Option<Box<Compacting>>,
    /// 摘要回复里调了工具、要改走隔离式（施工 6-6 下）。
    pub(super) isolating: bool,
}

impl Session {
    /// 打断在路上的请求：收到的半截照累积器留下，不是空的写成回复，多写一格被打断；
    /// 记一条结果是被打断的 `model.called`。返回追加的事件和半截回复里留下的调用。
    pub(in crate::session) fn cut_off(
        &mut self,
        at: Timestamp,
        call: Call,
        cause: Option<CommandId>,
    ) -> (Vec<Event>, Vec<ToolCall>) {
        let settled = self.settle(at, call, cause, Ending::CutOff);
        (settled.events, settled.calls)
    }

    /// 撤掉一个字都还没回的请求（施工 4-15，`supersede.rs`）：记一条结果是撤掉重发的 `model.called`，返回它。
    pub(in crate::session) fn superseded(
        &mut self,
        at: Timestamp,
        call: Call,
        cause: Option<CommandId>,
    ) -> Vec<Event> {
        self.settle(at, call, cause, Ending::Superseded).events
    }

    /// 这次请求有了结局：该写的回复写上，记一条 `model.called`。
    pub(super) fn settle(
        &mut self,
        at: Timestamp,
        call: Call,
        cause: Option<CommandId>,
        ending: Ending,
    ) -> Settled {
        let Call {
            seen,
            messages,
            difference,
            sent,
            first_token,
            accumulator,
            spans,
            compaction,
        } = call;
        let (usage, cost, mut error, stopped) = match ending {
            Ending::Said { usage, cost, error } => (usage, cost, error, None),
            Ending::CutOff => (None, None, None, Some(CallResult::Interrupted)),
            Ending::Superseded => (None, None, None, Some(CallResult::Superseded)),
        };
        let cut = stopped.is_some();
        let mut events = Vec::new();
        let mut reply = None;
        let mut calls = Vec::new();
        let mut summary = None;
        let mut isolating = false;
        let mut times = None;
        match &sent {
            None if !cut && error.is_none() => {
                error = Some(bad_stream("请求还没发出去就说完了"));
            }
            // 摘要请求不写回复，说完了的取出摘要（施工 6-2 上）。
            Some(_) if compaction.is_some() => {
                if !cut && error.is_none() {
                    let blocks = accumulator.finish(self.ledger.next_seq());
                    let isolates = compaction
                        .as_deref()
                        .is_some_and(|compacting| self.can_isolate(compacting));
                    match self.summary_of(&blocks, isolates) {
                        Ok(text) => summary = Some(text),
                        Err(bad) => {
                            isolating = isolates && called_tool(&blocks);
                            error = Some(bad);
                        }
                    }
                }
            }
            Some(sent) => {
                let seq = self.ledger.next_seq();
                // 出错断了的，照打断的规矩留下半截，工具调用一个不留：没收全的执行不了，收全了的
                // 也不派，回复没说完（02 第六节「回复怎么收」第 4 条）。
                let failed = error.is_some();
                let mut numbered = accumulator.numbered(seq, !cut && !failed);
                if failed {
                    numbered.retain(|(_, block)| !matches!(block, Block::ToolCall(_)));
                }
                let (kept, blocks): (Vec<usize>, Vec<Block>) = numbered.into_iter().unzip();
                if blocks.is_empty() {
                    if !cut && !failed {
                        error = Some(CallError {
                            class: ErrorClass::EmptyReply,
                            message: "回复里一个块都没有".to_string(),
                            status: None,
                        });
                    }
                } else {
                    calls = blocks
                        .iter()
                        .filter_map(|block| match block {
                            Block::ToolCall(call) => Some(call.clone()),
                            _ => None,
                        })
                        .collect();
                    let body = MessageAssistant {
                        blocks,
                        seen,
                        interrupted: cut || failed,
                        indexes: MessageAssistant::indexes_for(&kept),
                    };
                    let by = By::Model(sent.model.clone());
                    events.push(self.record(at, by, cause.clone(), Body::MessageAssistant(body)));
                    reply = Some(seq);
                    times = Some(spans.of(sent.at, kept));
                }
            }
            None => {}
        }
        let result = match stopped {
            Some(result) => result,
            None if error.is_some() => CallResult::Error,
            None => CallResult::Ok,
        };
        let called = ModelCalled {
            seen,
            endpoint: sent.as_ref().map(|sent| sent.model.endpoint.clone()),
            model: sent.as_ref().map(|sent| sent.model.model.clone()),
            request: sent.as_ref().map(|sent| sent.request.clone()),
            messages: messages as u64,
            first_difference: difference
                .map(|difference| Box::new(FirstDifference::from(difference))),
            usage,
            cost,
            first_token_ms: sent
                .as_ref()
                .zip(first_token)
                .map(|(sent, first)| millis(sent.at, first)),
            duration_ms: sent.as_ref().map(|sent| millis(sent.at, at)),
            blocks: times,
            result,
            error: error.clone(),
            compaction: compaction
                .as_ref()
                .map(|compacting| compacting.trigger().clone()),
            purpose: None,
        };
        let summary = summary
            .zip(compaction.as_ref())
            .map(|(summary, compacting)| {
                let (paths, reread) = compacting.rebuild_inputs();
                Summarized {
                    upto: compacting.upto(),
                    trigger: compacting.trigger().clone(),
                    instructions: compacting.instructions(),
                    refills: compacting.refills(),
                    cut: compacting.shortened().0,
                    summary,
                    before: compacting.before(),
                    usage: called.usage,
                    duration_ms: called.duration_ms,
                    paths,
                    reread,
                    prepared: false,
                }
            });
        events.push(self.record(at, By::Kernel, cause, Body::ModelCalled(called)));
        Settled {
            events,
            reply,
            calls,
            error,
            summary,
            compaction,
            isolating,
        }
    }
}
