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
        .call("p1", "package.remove", json!({"package": "base-memory"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let reply = client
        .call("t3", "memory.turns", json!({"query": "猫"}))
        .await;
    assert_eq!(reason(&reply), Some("memory_not_installed"), "{reply}");
}

/// 导进来的只读存档（施工 SE-1 下，设计 58 第二节第 8 条）：不载入也搜得到它以前的对话，照它快照里的人格进那一间。
#[tokio::test]
async fn an_imported_archive_is_found_without_being_loaded() {
    let (source, target) = (Home::new(), Home::new());
    let script = Script::new([Play::Says("记住了，你用 N 卡。")]);
    let mut client = Client::connect(with_persona(&source, &script, Catalog::default()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "我的显卡是 N 卡").await;
    source.until_turns(&session, 1).await;
    let bundle = source.work.join("archive.zip");
    let exported = client
        .call(
            "e1",
            "session.export",
            json!({"session": session, "format": "bundle", "to": {"path": bundle}}),
        )
        .await;
    assert!(exported.get("error").is_none(), "{exported}");

    let mut client = Client::connect(with_persona(&target, &Script::new([]), Catalog::default()));
    client.hello().await;
    let none = client
        .call("t-before", "memory.turns", json!({"query": "显卡"}))
        .await;
    assert_eq!(none["result"]["turns"], json!([]), "{none}");
    let imported = client
        .call(
            "i1",
            "session.import",
            json!({"path": bundle, "archive": true}),
        )
        .await;
    assert_eq!(
        imported["result"]["sessions"],
        json!([session]),
        "{imported}"
    );

    let found = until_found(&mut client, json!({"query": "显卡"})).await;
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["session"], session.as_str());
    assert_eq!(found[0]["text"], "我的显卡是 N 卡\n\n记住了，你用 N 卡。");
    // 只接回合库，不抽取：存档里的话不变成记下的记忆。
    let memories = client.call("l1", "memory.list", json!({})).await;
    assert_eq!(memories["result"]["memories"], json!([]), "{memories}");
}
