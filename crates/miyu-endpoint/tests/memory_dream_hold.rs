//! 叫停整理，剧本的一次性入口卡住不答（施工 test-once 补，`docs/construction/R-22-叫停整理.md`）：测试核心不起假服务器，
//! 整理合并的那一问卡在 `Once::Holds`，`/dream stop` 把它丢掉。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Once, Play, Script};

use crate::support::*;

fn line(id: &str, method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

/// 抽取交回的：四十轮都是同一条记忆（和 `memory_dream` 的 `every_turn` 一样）。
fn extracted() -> String {
    let memories: Vec<Value> = (1..=40)
        .map(|turn| json!({"class": "user", "text": "用户养了一只猫", "turn": turn}))
        .collect();
    json!({ "memories": memories }).to_string()
}

/// 整理时合并的那一问卡住，`/dream stop` 叫停以后 `/dream` 照样回应、说中断了，没有卡死。
#[tokio::test]
async fn a_held_one_shot_is_dropped_when_the_dream_is_stopped() {
    let script = Script::new([Play::Says("好。")]).once([
        Once::from(extracted()),
        Once::from(json!({"keywords": []}).to_string()),
        Once::Holds,
    ]);
    let home = Home::new();
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client
        .create_with(
            "c1",
            json!({"cwd": "~", "persona": "engineer", "preset": "full"}),
        )
        .await;
    client.say("s1", &session, "我养了一只猫").await;
    home.until_turns(&session, 1).await;
    let dream = json!({"session": session, "text": "/dream"});
    client.line(&line("k1", "command.run", dream)).await;
    // 三问都到了：最后一问是卡住的合并。
    until("合并的请求发出去了", || script.asked().len() >= 3).await;
    let stopping = json!({"session": session, "text": "/dream stop"});
    client.line(&line("k2", "command.run", stopping)).await;
    // 两条回应谁先到不一定。
    let mut replies = Vec::new();
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while replies.len() < 2 {
            let message = client.next().await.expect("连着");
            if matches!(message["id"].as_str(), Some("k1" | "k2")) {
                replies.push(message);
            }
        }
    })
    .await;
    assert!(waited.is_ok(), "叫停以后没卡死：{replies:?}");
    let reply = |id: &str| {
        replies
            .iter()
            .find(|reply| reply["id"] == id)
            .expect("回了")
    };
    assert_eq!(reply("k2")["result"]["said"], "已停止整理。", "{replies:?}");
    // 末尾写用时（施工 R-22 再补），每次不同，只看头尾。
    let said = reply("k1")["result"]["said"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        said.starts_with("整理已中断，用时 ") && said.ends_with("s，下次再整理。"),
        "{replies:?}"
    );
}
