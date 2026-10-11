//! 想起过什么（施工 R-4 下，`docs/blueprint/memory.md` 第八条，`protocol.md` 的 `memory.*` 第 1 条）：真核心、真回合库。
//! `memory.list`、`memory.search` 的一条带用到几次、最近一次；常驻那一块带过的算一次，没用过的是 0、`null`。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::memories::with_persona;
use crate::support::*;

/// 列出来的那一条（照编号找）。
fn entry<'a>(reply: &'a Value, id: &str) -> &'a Value {
    reply["result"]["memories"]
        .as_array()
        .and_then(|memories| memories.iter().find(|memory| memory["id"] == id))
        .unwrap_or_else(|| panic!("有 {id}：{reply}"))
}

#[tokio::test]
async fn a_memory_the_block_brought_shows_how_often_and_when() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(with_persona(&home, &script, Catalog::default()));
    client.hello().await;
    client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "用户养了一只猫"}),
        )
        .await;
    let listed = client.call("l1", "memory.list", json!({})).await;
    assert_eq!(entry(&listed, "m1")["used"], 0, "{listed}");
    assert_eq!(entry(&listed, "m1")["used_at"], Value::Null);

    let session = client.create("c2", "~").await;
    client.say("c3", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let injected = logged(&home, &session)
        .into_iter()
        .find(|event| event["kind"] == "context.injected" && event["by"]["id"] == "memory")
        .expect("常驻那一块带了它");
    // 日志先落盘、回合库后写：等它写进去（最多十秒）。
    let mut listed = Value::Null;
    for n in 0..500 {
        listed = client
            .call(&format!("l{n}"), "memory.list", json!({}))
            .await;
        if entry(&listed, "m1")["used"] == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(entry(&listed, "m1")["used"], 1, "{listed}");
    assert_eq!(entry(&listed, "m1")["used_at"], injected["at"]);

    client
        .call(
            "c4",
            "memory.remember",
            json!({"class": "user", "text": "用户住在杭州"}),
        )
        .await;
    let found = client
        .call("s1", "memory.search", json!({"query": "杭州 猫"}))
        .await;
    assert_eq!(entry(&found, "m2")["used"], 0, "{found}");
    assert_eq!(entry(&found, "m2")["used_at"], Value::Null);
    assert_eq!(entry(&found, "m1")["used"], 1, "搜的也带");
}
