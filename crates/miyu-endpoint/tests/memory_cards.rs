//! 人卡经协议（施工 E-1 下，设计 42，`docs/reference/memory.md`）：`memory.card_update` 写一张（去空白；正文太长的
//! `memory_too_long`，空的、叫法不合的参数不对），`memory.cards` 一人一张照最新的列；`memory.card_forget` 作废，没有这个人的
//! `unknown_memory`，作废的只在 `forgotten` 时列出来；同一个命令编号再来不重写。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::memories::connected;
use crate::support::*;

async fn cards(client: &mut Client, id: &str, params: Value) -> Vec<Value> {
    let listed = client.call(id, "memory.cards", params).await;
    listed["result"]["cards"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("列得出：{listed}"))
}

#[tokio::test]
async fn cards_are_written_listed_and_retired() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    assert!(cards(&mut client, "l0", json!({})).await.is_empty());

    // 一张最多 400 字（设计 42 PN4，2026-10-26 改，施工 E-2）：401 字的太长（400 字的收见下一条）。
    let too_long = "长".repeat(401);
    let reply = client
        .call(
            "w0",
            "memory.card_update",
            json!({"who": "qq:2", "text": too_long}),
        )
        .await;
    assert_eq!(reason(&reply), Some("memory_too_long"), "{reply}");
    for (n, bad) in [
        json!({"who": " ", "text": "同学"}),
        json!({"who": "qq:2", "text": " "}),
        json!({"who": "qq:2", "text": "同学", "names": [""]}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(&format!("b{n}"), "memory.card_update", bad)
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }

    let first = json!({"who": " qq:2 ", "text": " 小林是群友。 ", "names": ["小林"]});
    let reply = client.call("w1", "memory.card_update", first.clone()).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    client
        .call(
            "w2",
            "memory.card_update",
            json!({"who": "qq:2", "text": "小林是群友，喜欢拍照。"}),
        )
        .await;
    // 同一个命令编号再来：交回头一次的，不重写。
    client.call("w1", "memory.card_update", first).await;
    let listed = cards(&mut client, "l1", json!({})).await;
    assert_eq!(listed.len(), 1, "一人一张：{listed:?}");
    assert_eq!(
        (
            &listed[0]["who"],
            &listed[0]["text"],
            &listed[0]["names"],
            &listed[0]["by"]
        ),
        (
            &json!("qq:2"),
            &json!("小林是群友，喜欢拍照。"),
            &json!([]),
            &json!("person")
        ),
        "照最新的那一张"
    );

    let reply = client
        .call("f0", "memory.card_forget", json!({"who": "qq:9"}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_memory"), "{reply}");
    let reply = client
        .call(
            "f1",
            "memory.card_forget",
            json!({"who": "qq:2", "why": "认错了"}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(
        cards(&mut client, "l2", json!({})).await.is_empty(),
        "作废的不列"
    );
    let all = cards(&mut client, "l3", json!({"forgotten": true})).await;
    assert_eq!(all[0]["retired"], json!("认错了"));
}

#[tokio::test]
async fn a_card_of_four_hundred_characters_is_taken_whole() {
    // 设计 42 PN4（2026-10-26 项目主人改成 400，施工 E-2）：熟人的印象写到 400 字，人写的也整张收。
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let full = "满".repeat(400);
    let reply = client
        .call(
            "w0",
            "memory.card_update",
            json!({"who": "qq:2", "text": full}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let listed = cards(&mut client, "l0", json!({})).await;
    assert_eq!(listed[0]["text"], json!(full));
}
