//! 记忆的关键词经协议（施工 R-15 上，`docs/reference/memory.md`）：`memory.remember` 带关键词、一条记忆交 `keywords`，照关键词搜得到；
//! `memory.update` 只写 `keywords` 的编号不变、写了 `text` 的换新编号、关键词带过去（空的是去掉）；太长的拒、说是哪一格；两样都
//! 没写的参数不对。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::memories::{connected, pairs, texts};
use crate::support::*;

/// 列出来的第一条（新的在前）的 `keywords`。
async fn keywords_of(client: &mut Client, id: &str) -> Value {
    let listed = client.call(id, "memory.list", json!({})).await;
    listed["result"]["memories"][0]["keywords"].clone()
}

#[tokio::test]
async fn keywords_are_kept_shown_searched_and_changed() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "用户用 N 卡", "keywords": " 显卡,英伟达 "}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    assert_eq!(
        keywords_of(&mut client, "l1").await,
        json!("显卡,英伟达"),
        "去掉前后空白"
    );
    let found = client
        .call("s1", "memory.search", json!({"query": "英伟达"}))
        .await;
    assert_eq!(
        texts(&found),
        pairs(&[("m1", "用户用 N 卡")]),
        "照关键词搜得到"
    );

    let reply = client
        .call(
            "u1",
            "memory.update",
            json!({"id": "m1", "keywords": "GPU,4090"}),
        )
        .await;
    assert_eq!(
        reply["result"],
        json!({"id": "m1"}),
        "只换关键词：编号不变 {reply}"
    );
    assert_eq!(keywords_of(&mut client, "l2").await, json!("GPU,4090"));

    let reply = client
        .call(
            "u2",
            "memory.update",
            json!({"id": "m1", "text": "用户用 4090"}),
        )
        .await;
    let changed = reply["result"]["id"]
        .as_str()
        .expect("有新编号")
        .to_string();
    assert_ne!(changed, "m1", "改了正文：新编号");
    assert_eq!(
        keywords_of(&mut client, "l3").await,
        json!("GPU,4090"),
        "关键词带过去"
    );

    let reply = client
        .call(
            "u3",
            "memory.update",
            json!({"id": changed, "text": "用户换了 AMD 的卡", "keywords": ""}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        keywords_of(&mut client, "l4").await,
        Value::Null,
        "空的是去掉"
    );

    let reply = client
        .call(
            "c2",
            "memory.remember",
            json!({"class": "user", "text": "喜欢火锅"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        keywords_of(&mut client, "l5").await,
        Value::Null,
        "没写的没有"
    );
}

#[tokio::test]
async fn too_long_or_nothing_to_change_is_refused() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "用户用 N 卡", "keywords": "词".repeat(201)}),
        )
        .await;
    assert_eq!(reason(&reply), Some("memory_too_long"), "{reply}");
    assert_eq!(reply["error"]["data"]["field"], "keywords");
    assert_eq!(reply["error"]["data"]["limit"], 200);
    assert_eq!(reply["error"]["data"]["chars"], 201);
    let reply = client
        .call(
            "c2",
            "memory.remember",
            json!({"class": "user", "text": "长".repeat(121)}),
        )
        .await;
    assert_eq!(reply["error"]["data"]["field"], "text", "{reply}");
    let reply = client
        .call(
            "c3",
            "memory.remember",
            json!({"class": "user", "text": "用户用 N 卡", "keywords": "词".repeat(200)}),
        )
        .await;
    assert_eq!(
        reply["result"],
        json!({"id": "m1"}),
        "正好 200 字的收：{reply}"
    );
    let reply = client
        .call("u1", "memory.update", json!({"id": "m1"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "两样都没写：{reply}");
    let reply = client
        .call(
            "u2",
            "memory.update",
            json!({"id": "m1", "keywords": "词".repeat(201)}),
        )
        .await;
    assert_eq!(reply["error"]["data"]["field"], "keywords", "{reply}");
}
