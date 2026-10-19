//! 叫停人叫的整理（施工 R-23，`docs/blueprint/memory.md` 第七条第 13 款，`protocol.md` 的 `memory.dream_stop`）：真核心，整理
//! 记忆照 `models.chat` 发给假服务器，合并的请求一直不回。
//!
//! `memory.dream_stop` 照人格找那一间，叫停这时人叫的那一趟：回 `{"stopped": true}`；被叫停的 `memory.dream` 照样回应，带
//! `"stopped": true`；记忆流推 `merge_failed`（`stopped`）；没在整理的回 `not_dreaming`；下次整理接着合。

use serde_json::{Value, json};

use miyu_http::testkit::{Piece, Server};

use crate::support::providers::{data, profiles, routed, said};
use crate::support::*;

fn line(id: &str, method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

#[tokio::test]
async fn dream_stop_stops_the_rooms_dream_from_anywhere() {
    let mut stalled = said("{}");
    stalled.body.insert(0, Piece::Stall);
    let server = Server::start(vec![
        stalled,
        // 下次整理：合了那一条，再补一次关键词（施工 R-15 下）。
        said(&json!({"summary": "用户养猫。"}).to_string()),
        said(&json!({"keywords": []}).to_string()),
    ])
    .await;
    let home = Home::new();
    home.write(
        "system/config.toml",
        &format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n",
            server.base_url
        ),
    );
    let mut client = Client::connect(routed(&home, &[], data(profiles(json!({})))));
    client.hello().await;
    let engineer = json!({"persona": "engineer"});
    let idle = client
        .call("s0", "memory.dream_stop", engineer.clone())
        .await;
    assert_eq!(reason(&idle), Some("not_dreaming"), "{idle}");
    let remembered = client
        .call(
            "c1",
            "memory.remember",
            json!({"persona": "engineer", "class": "user", "text": "用户养了一只猫"}),
        )
        .await;
    assert!(remembered["result"]["id"].is_string(), "{remembered}");
    let memory = json!({"stream": "memory", "persona": "engineer"});
    client.line(&line("m1", "subscribe", memory)).await;
    client.until_reply("m1").await;
    client
        .line(&line("d1", "memory.dream", engineer.clone()))
        .await;
    server.wait_stalled(1).await;
    // 在后台答的 `memory.dream` 不挡同一个连接后面的请求。
    client
        .line(&line("s1", "memory.dream_stop", engineer.clone()))
        .await;
    let (mut pushed, mut replies) = (Vec::new(), Vec::new());
    while replies.len() < 2 {
        let message = client.next().await.expect("连着");
        match message["id"].as_str() {
            Some("d1" | "s1") => replies.push(message),
            _ => pushed.push(message),
        }
    }
    let reply = |id: &str| {
        replies
            .iter()
            .find(|reply| reply["id"] == id)
            .expect("回了")
            .clone()
    };
    assert_eq!(reply("s1")["result"], json!({"stopped": true}));
    assert_eq!(
        reply("d1")["result"],
        json!({"given": 0, "revised": 0, "retired": 0, "summary": false, "stopped": true}),
        "正在合的那一批放弃了"
    );
    // 记忆流的推送另走转发任务，可能晚于回应到。
    while !pushed
        .iter()
        .any(|push: &Value| push.to_string().contains(r#""why":"stopped""#))
    {
        pushed.push(client.next().await.expect("记忆流推了放弃的那一批"));
    }
    client
        .line(&line("s2", "memory.dream_stop", engineer.clone()))
        .await;
    let (_, again) = client.until_reply("s2").await;
    assert_eq!(
        reason(&again),
        Some("not_dreaming"),
        "整理回了就不算在跑：{again}"
    );
    client.line(&line("d2", "memory.dream", engineer)).await;
    let (_, next) = client.until_reply("d2").await;
    assert_eq!(
        next["result"],
        json!({"given": 1, "revised": 0, "retired": 0, "summary": true}),
        "下次接着合放弃的那一条：{next}"
    );
}

/// 会话里叫的那一趟（带 `session` 的 `memory.dream`，同 `/dream`）也停得下：`memory.dream_stop` 照会话找那一间。
#[tokio::test]
async fn dream_stop_reaches_a_dream_asked_from_a_session() {
    let mut stalled = said("{}");
    stalled.body.insert(0, Piece::Stall);
    let memories: Vec<Value> = (1..=40)
        .map(|turn| json!({"class": "user", "text": "用户养了一只猫", "turn": turn}))
        .collect();
    let server = Server::start(vec![
        said("好。"),
        said("养猫"),
        said(&json!({ "memories": memories }).to_string()),
        said(&json!({"keywords": []}).to_string()),
        stalled,
    ])
    .await;
    let home = Home::new();
    home.write(
        "system/config.toml",
        &format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n",
            server.base_url
        ),
    );
    let mut client = Client::connect(routed(&home, &[], data(profiles(json!({})))));
    client.hello().await;
    let session = client
        .create_with(
            "c1",
            json!({"cwd": "~", "persona": "engineer", "preset": "full"}),
        )
        .await;
    client.say("s1", &session, "我养了一只猫").await;
    until("起好标题", || {
        home.log(&session)
            .iter()
            .any(|event| matches!(event.body, miyu_kernel::event::Body::MetaChanged(_)))
    })
    .await;
    let at = json!({"session": session});
    client.line(&line("d1", "memory.dream", at.clone())).await;
    server.wait_stalled(1).await;
    client.line(&line("x1", "memory.dream_stop", at)).await;
    let mut replies = Vec::new();
    while replies.len() < 2 {
        let message = client.next().await.expect("连着");
        if matches!(message["id"].as_str(), Some("d1" | "x1")) {
            replies.push(message);
        }
    }
    for reply in &replies {
        assert_eq!(reply["result"]["stopped"], true, "{reply}");
    }
}
