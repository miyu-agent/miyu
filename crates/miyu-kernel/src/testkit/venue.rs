//! 场所会话的两样（施工 RT-1）：旁听记下一句、照记下的几条开一轮（`session.respond`），可以带接在最末尾的那一句。群里指定的人
//! 说的一句、带桥交的几块事实开一轮（施工 FT-1：照线上的样子造一次群聊的请求）。

use super::Stage;
use super::stage::text;
use crate::event::{Body, ContextInjected, VenueMessage};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::session::Command;

impl Stage {
    /// 旁听记下一句（不开回合），交回它的序号。
    ///
    /// # Panics
    ///
    /// 内核没记成 `message.user`。
    pub fn overhear(&mut self, words: &str) -> Seq {
        let venue = VenueMessage {
            msg: format!("venue-{}", self.log().len()),
            ..VenueMessage::default()
        };
        self.overhear_from(self.by.clone(), venue, words)
    }

    /// `by` 说的一句旁听记下（不开回合），平台的那几格照 `venue`（旁听这一格这里写上），交回它的序号（施工 FT-1）。
    ///
    /// # Panics
    ///
    /// 内核没记成 `message.user`。
    pub fn overhear_from(&mut self, by: By, venue: VenueMessage, words: &str) -> Seq {
        self.command_as(
            by,
            Command::Send {
                blocks: text(words),
                urgent: false,
                venue: Some(VenueMessage {
                    ambient: true,
                    ..venue
                }),
            },
        );
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

    /// 照记下的 `to` 开一轮（正在跑的并进去），带桥交的几块事实 `facts`（施工 FT-1）。
    pub fn respond_with(&mut self, to: &[Seq], facts: Vec<ContextInjected>) -> CommandId {
        self.command(Command::Respond {
            to: to.to_vec(),
            facts,
            tail: None,
        })
    }
}
