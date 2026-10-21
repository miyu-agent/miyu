//! 认识的人经协议（施工 E-1 上，设计 42，`docs/reference/memory.md`）：`memory.remember` 的 `person` 要写 `who`，没写的参数
//! 不对；一条记忆交 `who`，照类列得出；`memory.update` 只写 `who` 的照旧文新记一条、换是谁，只改正文的带上旧的是谁；别的类
//! 写了 `who` 也不收。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::memories::connected;
use crate::support::*;

/// 列出来的第一条（新的在前）。
async fn first(client: &mut Client, id: &str, params: Value) -> Value {
    let listed = client.call(id, "memory.list", params).await;
    listed["result"]["memories"][0].clone()
}

#[tokio::test]
async fn a_person_is_kept_with_who() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "person", "text": "小林用 RX 7800 XT"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "没写是谁：{reply}");

    let reply = client
        .call(
            "c2",
            "memory.remember",
            json!({"class": "person", "text": "小林用 RX 7800 XT", "who": " qq:2 "}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    client
        .call(
            "c3",
            "memory.remember",
            json!({"class": "user", "text": "用户用 N 卡", "who": "小林"}),
        )
        .await;
    assert_eq!(
        first(&mut client, "l1", json!({})).await["who"],
        Value::Null,
        "别的类写了是谁也不收"
    );
    let person = first(&mut client, "l2", json!({"class": "person"})).await;
    assert_eq!(
        (&person["id"], &person["class"], &person["who"]),
        (&json!("m1"), &json!("person"), &json!("qq:2")),
        "照类列得出、带是谁"
    );

    let reply = client
        .call("u1", "memory.update", json!({"id": "m1", "who": "qq:3"}))
        .await;
    let changed = reply["result"]["id"]
        .as_str()
        .expect("有新编号")
        .to_string();
    let person = first(&mut client, "l3", json!({"class": "person"})).await;
    assert_eq!(
        (&person["id"], &person["text"], &person["who"]),
        (&json!(changed), &json!("小林用 RX 7800 XT"), &json!("qq:3")),
        "只换是谁：照旧文新记一条"
    );

    client
        .call(
            "u2",
            "memory.update",
            json!({"id": changed, "text": "小林换了 9070 XT"}),
        )
        .await;
    let person = first(&mut client, "l4", json!({"class": "person"})).await;
    assert_eq!(
        (&person["text"], &person["who"]),
        (&json!("小林换了 9070 XT"), &json!("qq:3")),
        "只改正文：是谁带过去"
    );
}
