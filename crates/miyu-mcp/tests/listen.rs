//! 进度和订阅（施工 PT-1 下，规范 2026-07-28「Progress」「Subscriptions」）：带进度令牌的调用一条条收到进度、`_meta` 照旧带版本；
//! 不要进度的不带令牌；新版服务声明了 `listChanged` 的开一条 `subscriptions/listen`，推来的变了数进 `changes`；没声明的不开；
//! 不认这个方法的只试一次；丢下客户端就取消订阅。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use miyu_mcp::testkit::{Kind, Listen, Script};
use miyu_mcp::{Content, Progress};

use crate::support::connected;

#[tokio::test]
async fn progress_comes_through_while_the_call_runs() {
    for script in [Script::modern(), Script::legacy(Kind::LegacyRefuses)] {
        let (client, fake) = connected(script).await;
        let seen: Arc<Mutex<Vec<Progress>>> = Arc::default();
        let kept = Arc::clone(&seen);
        let called = client
            .call_reporting("steps", json!({}), move |progress| {
                kept.lock().unwrap().push(progress)
            })
            .await
            .expect("答得上");
        assert_eq!(called.content, [Content::Text("stepped".to_string())]);
        let said: Vec<String> = seen.lock().unwrap().iter().map(Progress::said).collect();
        assert_eq!(said, ["1/3 one", "2/3 two"]);
        let sent = fake
            .received()
            .into_iter()
            .find(|message| message["params"]["name"] == "steps")
            .expect("调过");
        assert!(
            sent["params"]["_meta"]["progressToken"].is_string(),
            "{sent}"
        );
        let quiet = client.call("steps", json!({})).await.expect("答得上");
        assert_eq!(quiet.content, [Content::Text("stepped".to_string())]);
        let plain = fake
            .received()
            .into_iter()
            .filter(|message| message["params"]["name"] == "steps")
            .nth(1)
            .expect("又调了一次");
        assert!(
            plain["params"]["_meta"].get("progressToken").is_none(),
            "不要进度的不带令牌：{plain}"
        );
    }
    let (client, fake) = connected(Script::modern()).await;
    client
        .call_reporting("steps", json!({}), |_| {})
        .await
        .expect("答得上");
    let sent = fake
        .received()
        .into_iter()
        .find(|message| message["params"]["name"] == "steps")
        .expect("调过");
    assert_eq!(
        sent["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"], "2026-07-28",
        "新版的版本和进度令牌并在一起：{sent}"
    );
}

/// 等到 `check` 成立，最多等 5 秒。
async fn eventually(check: impl Fn() -> bool) -> bool {
    for _ in 0..250 {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test]
async fn a_modern_server_with_list_changed_gets_a_subscription() {
    let script = Script {
        listen: Listen::On,
        ..Script::modern()
    };
    let (client, fake) = connected(script).await;
    let mut changes = client.changes();
    tokio::time::timeout(Duration::from_secs(5), changes.changed())
        .await
        .expect("五秒内收到变了")
        .expect("连接还在");
    let listened = fake
        .received()
        .into_iter()
        .find(|message| message["method"] == "subscriptions/listen")
        .expect("开了订阅");
    assert_eq!(
        listened["params"]["notifications"],
        json!({"toolsListChanged": true})
    );
    assert_eq!(
        listened["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
        "2026-07-28"
    );
    let id = listened["id"].clone();
    drop(client);
    assert!(
        eventually(|| fake.received().iter().any(|message| {
            message["method"] == "notifications/cancelled" && message["params"]["requestId"] == id
        }))
        .await,
        "丢下客户端就取消订阅"
    );
}

#[tokio::test]
async fn no_subscription_without_list_changed_and_one_try_when_refused() {
    let (client, fake) = connected(Script::modern()).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !fake.methods().contains(&"subscriptions/listen".to_string()),
        "没声明的不开"
    );
    drop(client);
    let script = Script {
        listen: Listen::Refused,
        ..Script::modern()
    };
    let (_client, fake) = connected(script).await;
    let listens = || {
        fake.methods()
            .iter()
            .filter(|method| *method == "subscriptions/listen")
            .count()
    };
    assert!(eventually(|| listens() == 1).await, "试了一次");
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert_eq!(listens(), 1, "不认的不再试");
}
