//! 场所会话的两样（施工 RT-1）：旁听记下一句、照记下的几条开一轮（`session.respond`），可以带接在最末尾的那一句。

use super::Stage;
use super::stage::text;
use crate::event::{Body, VenueMessage};
use crate::id::{CommandId, Seq};
use crate::session::Command;

impl Stage {
    /// 旁听记下一句（不开回合），交回它的序号。
    ///
    /// # Panics
    ///
    /// 内核没记成 `message.user`。
    pub fn overhear(&mut self, words: &str) -> Seq {
        let msg = format!("venue-{}", self.log().len());
        self.command(Command::Send {
            blocks: text(words),
            urgent: false,
            venue: Some(VenueMessage {
                msg,
                ambient: true,
                ..VenueMessage::default()
            }),
        });
        self.log()
            .iter()
            .rev()
            .find(|event| matches!(event.body, Body::MessageUser(_)))
            .map(|event| event.seq)
            .unwrap_or_else(|| panic!("旁听的那一句没记成 message.user"))
    }

    /// 照记下的 `to` 开一轮（正在跑的并进去），带接在每次请求最末尾的 `tail`。
    pub fn respond(&mut self, to: &[Seq], tail: Option<&str>) -> CommandId {
        self.command(Command::Respond {
            to: to.to_vec(),
            facts: Vec::new(),
            tail: tail.map(str::to_string),
        })
    }
}
