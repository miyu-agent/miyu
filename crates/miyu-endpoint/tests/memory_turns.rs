//! 搜以前的对话（施工 R-12 下，`docs/blueprint/memory.md`「协议」，`protocol.md` 的 `memory.turns`）：真核心、真回合库。
//!
//! 搜得到以前那一轮：会话、轮、时刻、人的话加她的回答；`limit`；另一间的搜不到；空的、写错的 `bad_params`，没装的
//! `memory_not_installed`。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::memories::with_persona;
use crate::support::*;

/// 搜到为止（最多十秒）：日志先落盘、回合库后写，看到这一轮结束时它可能还没进回合库。
async fn until_found(client: &mut Client, params: Value) -> Vec<Value> {
    for n in 0..500 {
        let reply = client
            .call(&format!("t{n}"), "memory.turns", params.clone())
            .await;
        let turns = reply["result"]["turns"]
            .as_array()
            .unwrap_or_else(|| panic!("有 turns：{reply}"))
            .clone();
        if !turns.is_empty() {
            return turns;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("十秒内回合库里没有")
}

/// 日志里 `kind` 那一条的序号。
fn seq_of(home: &Home, session: &str, kind: &str) -> u64 {
    logged(home, session)
        .iter()
        .find(|event| event["kind"] == kind)
        .and_then(|event| event["seq"].as_u64())
        .expect("有这一条")
}

#[tokio::test]
async fn a_past_turn_is_found_with_where_and_when() {
    let home = Home::new();
    let script = Script::new([Play::Says("记住了，你用 N 卡。"), Play::Says("好。")]);
    let mut client = Client::connect(with_persona(&home, &script, Catalog::default()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "我的显卡是 N 卡").await;
    home.until_turns(&session, 1).await;
    client.say("c3", &session, "今天天气不错").await;
    home.until_turns(&session, 2).await;

    let found = until_found(&mut client, json!({"query": "显卡"})).await;
    assert_eq!(found.len(), 1, "{found:?}");
    let turn = &found[0];
    assert_eq!(turn["session"], session.as_str());
    assert_eq!(turn["turn"], seq_of(&home, &session, "turn.started"));
    assert_eq!(turn["text"], "我的显卡是 N 卡\n\n记住了，你用 N 卡。");
    assert!(
        turn["at"].as_str().is_some_and(|at| at.ends_with('Z')),
        "{turn}"
    );

    until_found(&mut client, json!({"query": "天气"})).await;
    let both = client
        .call(
            "l1",
            "memory.turns",
            json!({"query": "显卡 天气", "limit": 1}),
        )
        .await;
    assert_eq!(
        both["result"]["turns"].as_array().map(Vec::len),
        Some(1),
        "{both}"
    );

    // 记忆只在会话里的会话有自己的回合库：人格那一间的这一轮不在里面。
    let created = client
        .call(
            "c4",
            "session.create",
            json!({"cwd": "~", "memory": "session"}),
        )
        .await;
    let own = created["result"]["session"].as_str().expect("造了");
    let elsewhere = client
        .call(
            "l2",
            "memory.turns",
            json!({"session": own, "query": "显卡"}),
        )
        .await;
    assert_eq!(elsewhere["result"], json!({"turns": []}), "{elsewhere}");
}

#[tokio::test]
async fn wrong_searches_are_refused() {
    let home = Home::new();
    let mut client = Client::connect(with_persona(&home, &Script::new([]), Catalog::default()));
    client.hello().await;
    for params in [
        json!({}),
        json!({"query": "  "}),
        json!({"query": 3}),
        json!({"query": "猫", "limit": 0}),
        json!({"query": "猫", "limit": 501}),
        json!({"query": "猫", "persona": "engineer", "session": "x"}),
    ] {
        let reply = client.call("t1", "memory.turns", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    let reply = client
        .call(
            "t2",
            "memory.turns",
            json!({"query": "猫", "persona": "nobody"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("unknown_persona"), "{reply}");
    let removed = client
        .call(
            "p1",
            "package.remove",
            json!({"package": "miyu-base-memory"}),
        )
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let reply = client
        .call("t3", "memory.turns", json!({"query": "猫"}))
        .await;
    assert_eq!(reason(&reply), Some("memory_not_installed"), "{reply}");
}
