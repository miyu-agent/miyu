//! 只看事件本身的几条规矩（施工 X-6 从 `ledger.rs` 挪出来，那边到了 500 行）：哪几种只在回合里发生，助手消息里工具调用的编号。

use crate::block::Block;
use crate::event::Body;
use crate::id::{CallId, Seq};

/// 只在回合里发生的种类：模型的回复、工具的结果、请人确认和人的决定、问人和人的回答、
/// 撤回排着队的消息、回合结束、暂停自动压缩、压缩。压缩跟着它所在的回合撤（施工 6-9）：不在回合里的压缩没有哪一轮撤得掉，
/// 它替代掉的回合也就撤不了。
pub(super) fn in_turn_only(body: &Body) -> bool {
    matches!(
        body,
        Body::MessageAssistant(_)
            | Body::TurnJoined(_)
            | Body::ToolResult(_)
            | Body::ApprovalRequested(_)
            | Body::ApprovalDecided(_)
            | Body::QuestionAsked(_)
            | Body::QuestionAnswered(_)
            | Body::MessageWithdrawn(_)
            | Body::TurnEnded(_)
            | Body::CompactionPaused(_)
            | Body::ContextCompacted(_)
    )
}

/// 块是工具调用的话，它的调用编号。
pub(super) fn tool_call_id(block: &Block) -> Option<CallId> {
    match block {
        Block::ToolCall(call) => Some(call.call_id),
        _ => None,
    }
}

/// 助手消息里第 k 个工具调用，编号是 `call_<这一条的序号>_<k>`（`03-事件模型.md` 第二节）。
pub(super) fn check_call_ids(seq: Seq, blocks: &[Block]) -> Result<(), String> {
    for (index, call) in (1..).zip(blocks.iter().filter_map(tool_call_id)) {
        if CallId::new(seq, index) != Some(call) {
            return Err(format!(
                "tool call {index} should have id call_{seq}_{index}, got {call}"
            ));
        }
    }
    Ok(())
}
