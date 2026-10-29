//! 人回答确认的命令（施工 M-7 上从 `approval.rs` 挪出来：那一份到了行数上限）。

use super::super::*;
use crate::event::Decision;
use crate::id::CallId;

/// 编号是 `n` 的命令：主人回答 `call_id` 的确认。
pub(in crate::session::tests) fn answer(
    n: u64,
    call_id: CallId,
    decision: Decision,
    reason: Option<&str>,
) -> Input {
    answer_nested(n, (call_id, None), decision, reason)
}

/// 同 [`answer`]，`nested` 是脚本里调的那一件的序号（施工 M-7 上），没写的答正在问的那一件。
pub(in crate::session::tests) fn answer_nested(
    n: u64,
    (call_id, nested): (CallId, Option<u32>),
    decision: Decision,
    reason: Option<&str>,
) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::Answer {
            call_id,
            answer: Answer::Approval {
                decision,
                reason: reason.map(str::to_string),
                attachments: Vec::new(),
                nested,
            },
        },
    })
}
