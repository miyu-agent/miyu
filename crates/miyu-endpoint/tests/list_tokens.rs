//! 会话列表的 `tokens` 一列（终端要的）：一个会话连同它的子会话一共用了多少 token（输入三项加输出，和终端底栏的 `Σ` 同一个
//! 算法），`session.list` 和 `sessions` 流都带；子会话用了，父会话那一项跟着推。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 剧本每说一句的用量：60 没命中、40 命中、10 输出。
const ONE_REPLY: u64 = 110;

/// 会话列表里编号是 `session` 的那一项的 `tokens`。
async fn tokens(client: &mut Client, session: &str) -> u64 {
    let reply = client.call("l", "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["session"] == session))
        .map(|one| one.get("tokens").map_or(0, |n| n.as_u64().expect("是数")))
        .unwrap_or_else(|| panic!("列表里有 {session}：{reply}"))
}

/// 读推送，直到编号是 `session` 的那一项的 `tokens` 是 `want`：最多 60 秒。
async fn pushed(client: &mut Client, session: &str, want: u64) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到 {session} 推 {want}"));
        if message["method"] == "sessions.changed"
            && message["params"]["session"] == session
            && message["params"]["entry"]["tokens"] == want
        {
            return message;
        }
    }
}

#[tokio::test]
async fn a_parent_counts_its_children_and_is_pushed_when_they_spend() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Says("一。"), Play::Says("二。")]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let parent = client.create("c", "~").await;
    assert_eq!(tokens(&mut client, &parent).await, 0, "还没说过话：不写");
    client.say("s1", &parent, "你好").await;
    home.until_turns(&parent, 1).await;
    assert_eq!(tokens(&mut client, &parent).await, ONE_REPLY);

    let forked = client
        .call("f", "session.fork", json!({"session": parent}))
        .await;
    let child = forked["result"]["session"]
        .as_str()
        .expect("分叉出来了")
        .to_string();
    let mut watching = Client::connect(Arc::clone(&core));
    watching.hello().await;
    watching
        .call("w", "subscribe", json!({"stream": "sessions"}))
        .await;
    client.say("s2", &child, "接着").await;
    home.until_turns(&child, 1).await;
    pushed(&mut watching, &child, ONE_REPLY).await;
    pushed(&mut watching, &parent, 2 * ONE_REPLY).await;
    assert_eq!(
        tokens(&mut client, &child).await,
        ONE_REPLY,
        "分叉的底子不算进子会话"
    );
    assert_eq!(
        tokens(&mut client, &parent).await,
        2 * ONE_REPLY,
        "父会话连同子会话"
    );
}

/// 会话列表的 `context`（施工 9-5 再补）：最近一次主请求的输入（没命中加命中），没说过话的不写；测试的模型端口查不到窗口，
/// 不写 `window`。
#[tokio::test]
async fn the_context_used_by_the_last_request_is_listed() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Says("一。")]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c", "~").await;
    let listed = |reply: &Value| {
        reply["result"]["sessions"]
            .as_array()
            .and_then(|all| {
                all.iter()
                    .find(|one| one["session"] == session.as_str())
                    .cloned()
            })
            .expect("列着")
    };
    let before = client.call("l1", "session.list", json!({})).await;
    assert!(listed(&before).get("context").is_none(), "还没说过话：不写");
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    let after = client.call("l2", "session.list", json!({})).await;
    assert_eq!(listed(&after)["context"], json!({"used": 100}), "{after}");
}
