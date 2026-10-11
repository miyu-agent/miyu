//! 一条私聊（`onebot.md` 第一条「怎么走」第 6 到 8 条，中间是「斜杠命令」；施工 O-36 从 `route.rs` 挪出来，那一份行数到了上限）：
//! 场所、平台上的人经群聊内核拼，拼不出来的记一行、这条不送；带上场所的格，`show_ids` 照这个私聊套出来的场所规则；白名单成员的
//! 私聊照场所规则带人格、预设、工作区（施工 O-27）；只有图、引用的照取图（施工 O-33），合并转发照群消息的写法（施工 O-34）。

use miyu_chat::VenueKind;
use serde_json::json;

use super::fields::{Flags, fields};
use super::forwards::{Forwards, with_placeholders};
use super::pictures::Pictures;
use super::session::Place;
use super::{Message, Route, applied};
use crate::TARGET;
use crate::core::Gone;
use crate::onebot::{Posted, command_id, person, venue};

impl Route {
    /// 一条私聊（第 6 到 8 条，中间是「斜杠命令」）。
    pub(super) async fn private(&mut self, posted: Posted) -> Result<(), Gone> {
        let made = venue(VenueKind::Private, posted.user)
            .and_then(|venue| Ok((venue, person(posted.user)?)));
        let (venue, external) = match made {
            Ok(made) => made,
            Err(error) => {
                tracing::warn!(target: TARGET, user = posted.user, message = posted.message_id, error = %error, "venue not made, message dropped");
                return Ok(());
            }
        };
        // 只有图、引用的私聊照取图（施工 O-33）；带了东西的、没有字也送（施工 O-35：她看得到记号、能取）。合并转发照群消息的写法
        // 在正文后面写占位、交以前展开（施工 O-34），只有转发的也送。
        let pictures = Pictures::of(posted.bot, posted.message_id, &posted.segments);
        let text = with_placeholders(&posted.text, posted.segments.forwards.len());
        if text.trim().is_empty() && pictures.own.is_empty() && !pictures.any() {
            tracing::debug!(target: TARGET, venue = %venue.id(), message = posted.message_id, "nothing to send");
            return Ok(());
        }
        // 私聊每一条都开回合：不写 `ambient`、`asleep`（「施工时定的」第 68 条）。
        let applied = self.applied(&venue);
        let flags = Flags {
            show_ids: applied::show_ids(&applied),
            ..Flags::default()
        };
        // 白名单成员的私聊会话归系统账号、照场所规则造（施工 O-27，同群）。
        let listed = self.whitelisted(&external).then_some(&applied);
        let message = Message {
            id: command_id(posted.bot, posted.message_id, posted.time),
            number: posted.message_id,
            text,
            acting: json!({"external": external}),
            fields: fields(&posted, &[], flags),
            place: Place::private(&venue, &external, (posted.bot, posted.user), listed),
            pictures: Some(pictures),
            forwards: Forwards::of(posted.bot, None, posted.message_id, &posted.segments),
        };
        self.submit(message).await.map(|_| ())
    }
}
