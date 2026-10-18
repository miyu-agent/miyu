//! 协议找得到无人格那一间（施工 R-12 再补，`docs/reference/memory.md`）：`persona` 写 `_none` 的，一个无人格的会话都没开过也列得出、
//! 记得下；无人格的会话里记的照 `_none` 看得到、照人格看不到，照 `_none` 记的无人格的会话看得到；`null` 照旧是没写（照默认人格）。

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::*;

fn texts(reply: &serde_json::Value) -> Vec<String> {
    reply["result"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("有 memories：{reply}"))
        .iter()
        .filter_map(|memory| memory["text"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn the_room_without_a_persona_is_reached_by_its_id() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client
        .call("l1", "memory.list", json!({"persona": "_none"}))
        .await;
    assert_eq!(
        texts(&reply),
        Vec::<String>::new(),
        "没开过无人格的会话也列得出：{reply}"
    );
    let reply = client
        .call(
            "r1",
            "memory.remember",
            json!({"persona": "_none", "class": "user", "text": "用户喜欢喝茶"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");

    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/remember 用户养猫"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let listed = client
        .call("l2", "memory.list", json!({"session": session}))
        .await;
    assert_eq!(
        texts(&listed),
        ["用户养猫", "用户喜欢喝茶"],
        "照 _none 记的，无人格的会话看得到"
    );
    let listed = client
        .call("l3", "memory.list", json!({"persona": "_none"}))
        .await;
    assert_eq!(
        texts(&listed),
        ["用户养猫", "用户喜欢喝茶"],
        "无人格的会话里记的，照 _none 看得到"
    );
    let listed = client
        .call("l4", "memory.list", json!({"persona": "engineer"}))
        .await;
    assert_eq!(texts(&listed), Vec::<String>::new(), "人格那一间没有");
    let reply = client
        .call("l5", "memory.list", json!({"persona": null}))
        .await;
    assert_eq!(
        reason(&reply),
        Some("memory_unavailable"),
        "null 是没写：照默认人格，没设的没有：{reply}"
    );
}
