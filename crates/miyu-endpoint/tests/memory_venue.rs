//! 在哪个场所说的（施工 R-13 上，`docs/blueprint/memory.md` 第九条，`protocol.md` 的 `memory.*` 第 1 条）：人经协议是主人那边，
//! 一个大脑全部列得出；一条带 `venue`，本机的是 `null`。发生过的事带涉及谁（施工 E-1 修四），没有的是空列表。

use serde_json::{Value, json};

use miyu_kernel::id::VenueId;
use miyu_recall::{MemoryEvent, Saved};
use miyu_session::testkit::Script;
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::Room;

use crate::support::memories::connected;
use crate::support::*;

#[tokio::test]
async fn the_owner_lists_what_was_said_anywhere_with_where() {
    let home = Home::new();
    // 核心开记忆日志以前，照数据根写进去一条群里说的。
    {
        let (log, _) = MemoryLogs::new(&home.root)
            .open(&Room::persona(&alice(), "engineer"))
            .expect("开得了");
        let saved = Saved {
            class: "episode".into(),
            text: "小林在学日语".into(),
            sources: Vec::new(),
            audience: Vec::new(),
            replaces: None,
            about: None,
            venue: Some(VenueId::parse("qq:group:1").expect("合写法")),
            keywords: None,
            who: None,
            people: vec!["qq:2".into()],
        };
        let at = miyu_kernel::time::Timestamp::parse("2026-10-11T08:00:00.000Z").expect("合写法");
        log.append(
            at,
            miyu_kernel::origin::By::Kernel,
            None,
            &MemoryEvent::Saved(saved),
        )
        .expect("记得下");
    }
    let mut client = connected(&home, &Script::new([])).await;
    client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "主人养了一只猫"}),
        )
        .await;
    let listed = client.call("l1", "memory.list", json!({})).await;
    let memories = listed["result"]["memories"].as_array().expect("有");
    let venue = |text: &str| {
        memories
            .iter()
            .find(|memory| memory["text"] == text)
            .map(|memory| memory["venue"].clone())
            .unwrap_or_else(|| panic!("有 {text}：{listed}"))
    };
    assert_eq!(venue("小林在学日语"), "qq:group:1");
    assert_eq!(venue("主人养了一只猫"), Value::Null);
    let people = |text: &str| {
        memories
            .iter()
            .find(|memory| memory["text"] == text)
            .map(|memory| memory["people"].clone())
    };
    assert_eq!(people("小林在学日语"), Some(json!(["qq:2"])));
    assert_eq!(people("主人养了一只猫"), Some(json!([])));
}
