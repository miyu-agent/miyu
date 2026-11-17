//! 场所里的人说的话（施工 VW-2）：`user` 条目的 `from` 照 `by` 写，再多一格 `name`（那条消息的 `venue.name`，只当显示名）；
//! 没有名字的不写；本机的人说的照旧没有 `from`。读回来一样。

use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_view::Projector;

use crate::support::*;

const CREATED: &str = r#"{"seq":1,"at":"2026-11-17T09:00:00.000Z","kind":"session.created","by":{"kind":"person"},"cause":"cmd-0","body":{"venue":"qq:group:777","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"full","read_only":false}}}"#;

/// 群里 `id` 说 `text`，平台的格照 `venue`（JSON 对象）。
fn said(seq: u64, id: &str, venue: &str) -> String {
    format!(
        r#"{{"seq":{seq},"at":"2026-11-17T09:00:0{seq}.000Z","kind":"message.user","by":{{"kind":"external","venue":"qq:group:777","id":"{id}","role":"member"}},"cause":"cmd-{seq}","body":{{"blocks":[{{"type":"text","text":"在吗"}}],"venue":{venue}}}}}"#
    )
}

#[test]
fn a_venue_message_carries_the_sender_name() {
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    for line in [
        CREATED.to_string(),
        said(
            2,
            "qq:20017",
            r#"{"msg":"8810","name":"小林<b>","ambient":true}"#,
        ),
        said(3, "qq:20018", r#"{"msg":"8811","ambient":true}"#),
    ] {
        projector.event(&Event::from_line(&line).unwrap_or_else(|e| panic!("{line}: {e}")));
    }
    let entries = projector.entries().to_vec();
    let users = of_kind(&entries, "user");
    let named = json(users[0]);
    assert_eq!(
        named["from"],
        serde_json::json!({"kind": "external", "venue": "qq:group:777", "id": "qq:20017", "role": "member", "name": "小林<b>"}),
        "名字原样，头照常转义"
    );
    let nameless = json(users[1]);
    assert_eq!(nameless["from"]["id"], "qq:20018");
    assert!(nameless["from"].get("name").is_none(), "没有名字的不写");
    for entry in &entries {
        round_trip(entry);
    }
}

/// 戳一戳（施工 VP-1补，VP-1 的场所格进视图）：`user` 条目多带 `poke`（只写 `true`）和 `poke_text`（平台上那半句，不可信，原样
/// 交头转义）；没戳的两格都不写。读回来一样。
#[test]
fn a_poke_carries_its_mark_and_text() {
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    let poked = format!(
        r#"{{"seq":2,"at":"2026-11-17T09:00:02.000Z","kind":"message.user","by":{{"kind":"external","venue":"qq:group:777","id":"qq:20017","role":"member"}},"cause":"cmd-2","body":{{"blocks":[],"venue":{}}}}}"#,
        r#"{"msg":"8812","name":"小林","poke":true,"poke_text":"的小脑袋<b>"}"#
    );
    for line in [
        CREATED.to_string(),
        poked,
        said(3, "qq:20018", r#"{"msg":"8813","name":"阿青"}"#),
    ] {
        projector.event(&Event::from_line(&line).unwrap_or_else(|e| panic!("{line}: {e}")));
    }
    let entries = projector.entries().to_vec();
    let users = of_kind(&entries, "user");
    let poke = json(users[0]);
    assert_eq!(
        (poke["poke"].clone(), poke["poke_text"].clone()),
        (serde_json::json!(true), serde_json::json!("的小脑袋<b>")),
        "原样，头照常转义：{poke}"
    );
    assert_eq!(poke["from"]["name"], "小林");
    let plain = json(users[1]);
    assert!(
        plain.get("poke").is_none() && plain.get("poke_text").is_none(),
        "没戳的不写：{plain}"
    );
    for entry in &entries {
        round_trip(entry);
    }
}
