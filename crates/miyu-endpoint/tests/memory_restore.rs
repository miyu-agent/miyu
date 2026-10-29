//! 恢复误删（施工 R-12 下，`docs/blueprint/memory.md` 第二条第 6 款，`protocol.md` 的 `memory.restore`）：真核心、真记忆日志。
//!
//! 忘了的恢复以后照常列出、`retired` 是 `null`、搜得到；同一个编号只算一次，本来就算数的回 `{}`、不写；订阅着的补得到
//! `ext.memory.restored`；改掉的、清掉的 `memory_not_current`，没有的 `unknown_memory`，写错的 `bad_params`，没装的
//! `memory_not_installed`。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::memories::{connected, pairs, texts};
use crate::support::*;

async fn restore(client: &mut Client, id: &str, memory: &str) -> Value {
    client
        .call(id, "memory.restore", json!({"id": memory}))
        .await
}

#[tokio::test]
async fn a_forgotten_memory_comes_back() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "用户养了一只猫"}),
        )
        .await;
    let forgot = client
        .call("c2", "memory.forget", json!({"id": "m1", "why": "不养了"}))
        .await;
    assert_eq!(forgot["result"], json!({}), "{forgot}");
    let listed = client.call("l1", "memory.list", json!({})).await;
    assert_eq!(texts(&listed), pairs(&[]));

    for _ in 0..2 {
        let reply = restore(&mut client, "c3", "m1").await;
        assert_eq!(reply["result"], json!({}), "{reply}");
    }
    let listed = client.call("l2", "memory.list", json!({})).await;
    assert_eq!(texts(&listed), pairs(&[("m1", "用户养了一只猫")]));
    assert_eq!(listed["result"]["memories"][0]["retired"], Value::Null);
    let found = client
        .call("l3", "memory.search", json!({"query": "猫"}))
        .await;
    assert_eq!(texts(&found), pairs(&[("m1", "用户养了一只猫")]));

    let again = restore(&mut client, "c4", "m1").await;
    assert_eq!(again["result"], json!({}), "本来就算数的：{again}");
    client
        .call("c6", "memory.forget", json!({"id": "m1"}))
        .await;
    let repeated = restore(&mut client, "c3", "m1").await;
    assert_eq!(repeated["result"], json!({}), "{repeated}");
    let listed = client.call("l4", "memory.list", json!({})).await;
    assert_eq!(
        texts(&listed),
        pairs(&[]),
        "做过的编号再来不再恢复：又忘了的还是忘了"
    );
    let next = client
        .call(
            "c5",
            "memory.remember",
            json!({"class": "user", "text": "二"}),
        )
        .await;
    assert_eq!(
        next["result"],
        json!({"id": "m5"}),
        "日志里只多了作废、恢复、作废三行：同一个编号只算一次，本来就算数的不写：{next}"
    );

    let line = json!({"jsonrpc": "2.0", "id": "s1", "method": "subscribe",
        "params": {"stream": "memory", "after": 2}});
    client.line(&line.to_string()).await;
    let (filled, _) = client.until_reply("s1").await;
    let event = &filled[0]["params"]["event"];
    assert_eq!(event["kind"], "ext.memory.restored", "{filled:?}");
    assert_eq!(event["body"], json!({"id": "m1"}));
    assert_eq!(event["cause"], "c3");
    let gone = client
        .call("u1", "unsubscribe", json!({"stream": "memory"}))
        .await;
    assert_eq!(gone["result"], json!({}), "{gone}");

    let cleared = client
        .call("c7", "memory.forget", json!({"clear": "me"}))
        .await;
    assert_eq!(cleared["result"], json!({"cleared": 2}), "{cleared}");
    let repeated = restore(&mut client, "c3", "m1").await;
    assert_eq!(
        repeated["result"],
        json!({}),
        "先认编号、再查参数：清掉了也照头一次回：{repeated}"
    );
}

#[tokio::test]
async fn what_cannot_be_restored_is_refused() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "一"}),
        )
        .await;
    client
        .call("c2", "memory.update", json!({"id": "m1", "text": "一改"}))
        .await;
    client
        .call("c3", "memory.forget", json!({"id": "m2"}))
        .await;
    let reply = restore(&mut client, "r1", "m1").await;
    assert_eq!(
        reason(&reply),
        Some("memory_not_current"),
        "改掉的旧版本：{reply}"
    );
    let cleared = client
        .call("c4", "memory.forget", json!({"clear": "me"}))
        .await;
    assert_eq!(cleared["result"], json!({"cleared": 1}), "{cleared}");
    let reply = restore(&mut client, "r2", "m2").await;
    assert_eq!(
        reason(&reply),
        Some("memory_not_current"),
        "清掉的：{reply}"
    );
    let reply = restore(&mut client, "r3", "m9").await;
    assert_eq!(reason(&reply), Some("unknown_memory"), "{reply}");
    for params in [json!({"id": "x"}), json!({}), json!({"id": 3})] {
        let reply = client.call("r4", "memory.restore", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    let removed = client
        .call(
            "p1",
            "package.remove",
            json!({"package": "miyu-base-memory"}),
        )
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let reply = restore(&mut client, "r5", "m2").await;
    assert_eq!(reason(&reply), Some("memory_not_installed"), "{reply}");
}
