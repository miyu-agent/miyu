//! 语音助手那头的主人（施工 VA-1，设计 `20-语音.md` 第九节）：平台是 `voice` 的桥报 `voice:owner`，不用写进对应表就是主人本人：
//! 私聊不关，说的话记成本人。别的平台的桥报同一个号不算（照场所的平台认），照外人关起来、记成外部身份。

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;

use crate::support::extensions::*;
use crate::support::served::{replies, served_core};
use crate::support::*;

/// 平台是 `platform` 的桥在场所 `venue` 开 `voice:owner` 的私聊、说一句：交回造会话那一条记的关不关，和那句话记成谁。
async fn opened(platform: &str, venue: &str) -> (bool, By) {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    let open = json!({"venue": venue, "kind": "private", "peer": "voice:owner"}).to_string();
    install_bridge_on(
        &home,
        "bridge",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("ask:venue.session:{open}"),
            r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"voice:owner"}}"#,
            "wait",
        ]),
        "always",
        Some(platform),
    );
    let _core = served_core(&home, &Script::new([Play::Says("在。")]));
    let got = replies(&path, 3).await;
    let session = got[1]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("造了会话：{got:?}"))
        .to_string();
    assert!(got[2].get("error").is_none(), "{got:?}");
    let dir = dir(&home, &session);
    let events = read_events(&dir).expect("读得了");
    let Body::SessionCreated(created) = &events[0].body else {
        panic!("第一条是 session.created");
    };
    let said = events
        .iter()
        .find(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.by.clone())
        .expect("记了那句话");
    (created.confined, said)
}

#[tokio::test]
async fn the_voice_bridge_s_owner_is_the_owner() {
    let (confined, said) = opened("voice", "voice:local").await;
    assert!(!confined, "主人的私聊不关");
    assert!(matches!(said, By::Person(_)), "记成本人：{said:?}");
}

#[tokio::test]
async fn another_bridge_cannot_claim_the_voice_owner() {
    let (confined, said) = opened("qq", "qq:private:1").await;
    assert!(confined, "不是 voice 的场所，照外人关起来");
    assert!(
        matches!(&said, By::External(external) if !external.owner),
        "记成外部身份、不是主人：{said:?}"
    );
}
