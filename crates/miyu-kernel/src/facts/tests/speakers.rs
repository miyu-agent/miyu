//! 开这一轮的人话是哪几个人说的（施工 E-0，设计 `42-认识的人.md`）：照记下的几条开的一轮照 `triggers`，别的照 `trigger`；通讯
//! 平台上的人交平台身份，对应表认出的主人标 `owner`，私聊里认出的本人交它经的平台身份；本机的头上说的、不是开这一轮的不算；
//! 同一个人只交一次，照最后一次开口排。

use super::*;
use crate::id::Seq;

/// 第 `seq` 条：`by` 说了一句。
fn said(seq: u64, by: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-10-21T07:00:{seq:02}.000Z","kind":"message.user","by":{by},"body":{{"blocks":[{{"type":"text","text":"第 {seq} 句"}}]}}}}"#
    ))
    .unwrap()
}

/// 第 `seq` 条：开一轮，`trigger`、`triggers` 照给的写。
fn started(seq: u64, trigger: Option<u64>, triggers: &[u64]) -> Event {
    let mut body = serde_json::json!({"triggers": triggers});
    if let Some(trigger) = trigger {
        body["trigger"] = serde_json::json!(trigger);
    }
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-10-21T07:00:{seq:02}.000Z","kind":"turn.started","turn":{seq},"by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .unwrap()
}

fn member(id: &str) -> String {
    format!(r#"{{"kind":"external","venue":"qq:group:1","id":"{id}","role":"member"}}"#)
}

fn history(events: Vec<Event>) -> History {
    let mut history = History::whole();
    for event in events {
        history.append(event);
    }
    history
}

fn ids(speakers: &[Speaker]) -> Vec<(&str, bool)> {
    speakers
        .iter()
        .map(|speaker| (speaker.id.as_str(), speaker.owner))
        .collect()
}

#[test]
fn a_group_turn_hands_each_speaker_once_latest_last() {
    let owner =
        r#"{"kind":"external","venue":"qq:group:1","id":"qq:1","owner":true,"role":"member"}"#;
    let history = history(vec![
        said(1, &member("qq:20017")),
        said(2, &member("qq:20018")),
        said(3, owner),
        said(4, &member("qq:20017")),
        said(5, &member("qq:20019")),
        started(6, Some(4), &[1, 2, 3, 4]),
    ]);
    assert_eq!(
        ids(&speakers(&history, TurnId::new(Seq::new(6).unwrap()))),
        [("qq:20018", false), ("qq:1", true), ("qq:20017", false)],
        "只算开这一轮的几条（5 号不算）；20017 说了两次，排在最后那次；主人标出来"
    );
}

#[test]
fn a_private_owner_hands_the_platform_id_and_a_local_head_nothing() {
    let via = r#"{"kind":"person","via":"qq:1"}"#;
    let local = r#"{"kind":"person"}"#;
    let history = history(vec![
        said(1, via),
        started(2, Some(1), &[]),
        said(3, local),
        started(4, Some(3), &[]),
    ]);
    assert_eq!(
        ids(&speakers(&history, TurnId::new(Seq::new(2).unwrap()))),
        [("qq:1", true)]
    );
    assert!(
        speakers(&history, TurnId::new(Seq::new(4).unwrap())).is_empty(),
        "本机的头上说的没有平台身份"
    );
    assert!(
        speakers(&history, TurnId::new(Seq::new(9).unwrap())).is_empty(),
        "没有这一轮"
    );
}
