//! 工具调用后的通知（施工 HK-2，设计 `65-工具挂接点.md` 第四节）：一次调用的结果落了盘以后，交给端口发 `tool.after`。只读：
//! 不产生事件、不碰请求（第五节），端口只入队，actor 不等。
//!
//! - 直接调的：她的回复（`message.assistant`）落盘时，表里有收件人的那几次调用记下工具名和参数原文（`tool_call` 块，同事件流
//!   里的）；那一次的 `tool.result` 落了盘，照它写一条，记下的拿掉。载入以前派的（重启时内核补的结果）这里没记，不发：
//!   `tool.after` 不保证送达，完整的要订事件流。
//! - 脚本里调的（`tool.nested`，core1 2026-11-07 照推荐定照发）：那一条自己带着工具名、修正过的参数，结果是 blob 的引用。
//!
//! 没有收件人的会话（表里没有对得上的）不记、不写，一点不多：每次调用查一次内存里的表。

use std::collections::BTreeMap;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::CallId;
use miyu_tool::{AfterNote, AfterOutcome};

use super::Actor;

/// 等着结果落盘、要发通知的直接调用：调用编号到工具名、参数原文。
#[derive(Default)]
pub(crate) struct Calls(BTreeMap<CallId, (String, String)>);

impl Actor {
    /// 这一批里要发的通知（还没落盘，落了盘再交给端口，`store.rs`）：见模块的说明。
    pub(super) fn afters(&mut self, events: &[Event]) -> Vec<AfterNote> {
        let Some((port, session)) = self.tools.hooks() else {
            return Vec::new();
        };
        let access = |name: &str| {
            self.guard
                .access_of(name)
                .map(|access| access.as_str().to_string())
        };
        let mut notes = Vec::new();
        for event in events {
            match &event.body {
                Body::MessageAssistant(reply) => {
                    for block in &reply.blocks {
                        if let Block::ToolCall(call) = block
                            && port.afters(&call.name, access(&call.name).as_deref())
                        {
                            self.afters
                                .0
                                .insert(call.call_id, (call.name.clone(), call.args.clone()));
                        }
                    }
                }
                Body::ToolResult(result) => {
                    let Some((tool, args)) = self.afters.0.remove(&result.call_id) else {
                        continue;
                    };
                    notes.push(AfterNote {
                        session: session.as_str().to_string(),
                        venue: self.tools.venue().as_str().to_string(),
                        confined: self.guard.confined(),
                        call_id: result.call_id.to_string(),
                        access: access(&tool),
                        tool,
                        status: result.status.as_str().to_string(),
                        duration_ms: result.duration_ms,
                        by: serde_json::to_value(&event.by).unwrap_or_default(),
                        args,
                        outcome: AfterOutcome::Blocks(
                            serde_json::to_value(&result.blocks).unwrap_or_default(),
                        ),
                    });
                }
                Body::ToolNested(nested)
                    if port.afters(&nested.name, access(&nested.name).as_deref()) =>
                {
                    notes.push(AfterNote {
                        session: session.as_str().to_string(),
                        venue: self.tools.venue().as_str().to_string(),
                        confined: self.guard.confined(),
                        call_id: nested.id.to_string(),
                        tool: nested.name.clone(),
                        access: access(&nested.name),
                        status: nested.status.as_str().to_string(),
                        duration_ms: nested.duration_ms,
                        by: serde_json::to_value(&event.by).unwrap_or_default(),
                        args: nested.args.clone(),
                        outcome: AfterOutcome::Stored {
                            result: nested.result.as_ref().map(ToString::to_string),
                            bytes: nested.bytes,
                        },
                    });
                }
                _ => {}
            }
        }
        notes
    }

    /// 落了盘：交给端口发出去（端口只入队，当场交回）。
    pub(super) fn send_afters(&self, notes: Vec<AfterNote>) {
        if notes.is_empty() {
            return;
        }
        if let Some((port, _)) = self.tools.hooks() {
            for note in notes {
                port.after(note);
            }
        }
    }
}
