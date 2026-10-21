//! 开这一轮的人话是哪几个人说的（施工 E-0，设计 `42-认识的人.md`）：回合开始交给挂接点，记忆照它认人。

use crate::event::Body;
use crate::history::History;
use crate::id::{ExternalId, TurnId};
use crate::origin::By;

/// 这一轮说话的一个人（施工 E-0，设计 `42-认识的人.md`）：平台身份，和是不是主人（群里对应表里有的外部身份、私聊里认出的
/// 本人）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Speaker {
    /// 平台身份：`qq:10086` 这种，跨场所不变。
    pub id: ExternalId,
    /// 是主人：对应表认出来的。
    pub owner: bool,
}

/// 开 `turn` 这一轮的人话是哪几个人说的（施工 E-0）：回合开始交给挂接点（`Action::RunTurnStartHooks` 的 `speakers`），记忆照它
/// 认人。照这一轮 `turn.started` 的 `triggers`（照记下的几条开的一轮），没有的照 `trigger`，在有效历史里找那几条
/// `message.user`：通讯平台上的人交平台身份，私聊里认出的本人交它经的那个平台身份；本机的头上说的、不是人说的没有。同一个人
/// 只交一次，照最后一次开口排，最近开口的在最后。
pub fn speakers(history: &History, turn: TurnId) -> Vec<Speaker> {
    let events = history.events();
    let Some(started) = events.iter().rev().find_map(|event| match &event.body {
        Body::TurnStarted(started) if event.seq == turn.started() => Some(started),
        _ => None,
    }) else {
        return Vec::new();
    };
    let opened: Vec<_> = match (started.triggers.is_empty(), started.trigger) {
        (false, _) => started.triggers.clone(),
        (true, Some(trigger)) => vec![trigger],
        (true, None) => return Vec::new(),
    };
    let mut speakers: Vec<Speaker> = Vec::new();
    for event in events {
        if !opened.contains(&event.seq) || !matches!(event.body, Body::MessageUser(_)) {
            continue;
        }
        let speaker = match &event.by {
            By::External(external) => Speaker {
                id: external.id.clone(),
                owner: external.account.is_some(),
            },
            By::Person(person) => match &person.via {
                Some(via) => Speaker {
                    id: via.clone(),
                    owner: true,
                },
                None => continue,
            },
            _ => continue,
        };
        speakers.retain(|seen| seen.id != speaker.id);
        speakers.push(speaker);
    }
    speakers
}
