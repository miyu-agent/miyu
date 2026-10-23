//! 替身上人回答确认、回答提问的命令（施工 M-7 上从 `stage.rs` 挪出来：那一份到了行数上限）。

use super::Stage;
use crate::event::{Decision, Response};
use crate::id::{CallId, CommandId};
use crate::session::{Answer, Command};

impl Stage {
    /// 回答调用 `call_id` 的确认。
    pub fn decide(
        &mut self,
        call_id: CallId,
        decision: Decision,
        reason: Option<&str>,
    ) -> CommandId {
        self.decide_nested(call_id, None, decision, reason)
    }

    /// 同 [`Self::decide`]，`nested` 是脚本里调的那一件的序号（施工 M-7 上）；没写的答正在问的那一件。
    pub fn decide_nested(
        &mut self,
        call_id: CallId,
        nested: Option<u32>,
        decision: Decision,
        reason: Option<&str>,
    ) -> CommandId {
        self.command(Command::Answer {
            call_id,
            answer: Answer::Approval {
                decision,
                reason: reason.map(str::to_string),
                attachments: Vec::new(),
                nested,
            },
        })
    }

    /// 回答调用 `call_id` 问的那组题。
    pub fn reply(&mut self, call_id: CallId, answers: Vec<Response>) -> CommandId {
        self.command(Command::Answer {
            call_id,
            answer: Answer::Questions(answers),
        })
    }
}
