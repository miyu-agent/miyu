//! 给人的通知（施工 J-3 下，`notices` 流）：每次新开会话的定时，开出来的那一轮答完了推 `notice {"kind": "schedule.ran"}`，
//! 带任务、会话、成没成；跑在已有会话上的不推；唤醒表坏了换一份空的推 `schedules.reset`。订阅回应是空的，扩展订不了。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::clock::{Hand, core_with};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 读到一条 `notice` 为止：别的消息跳过，最多 60 秒。
async fn notice(client: &mut Client) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到通知"));
        if message["method"] == "notice" {
            return message["params"].clone();
        }
    }
}

#[tokio::test]
async fn a_new_session_s_run_is_told_when_it_ends() {
    let home = Home::new();
    let script = Script::new([Play::Says("在跑的会话那一句。"), Play::Says("早。")]);
    let hand = Hand::at(START);
    let core = core_with(&home, &script, &hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    core.start_schedules();
    let subscribed = client
        .call("n", "subscribe", json!({"stream": "notices"}))
        .await;
    assert_eq!(subscribed["result"], json!({}), "{subscribed}");

    // 跑在已有会话上的先到点：不推。
    let on = client
        .call(
            "a1",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "看看", "target": {"session": session}}),
        )
        .await;
    assert_eq!(on["result"]["schedule"]["id"], "t1", "{on}");
    hand.advance(600_000);
    home.until_turns(&session, 1).await;
    let paused = client
        .call("p", "schedule.pause", json!({"id": "t1"}))
        .await;
    assert!(paused["result"].is_object(), "{paused}");

    let at = START + 600_000 + 30_000;
    let added = client
        .call(
            "a2",
            "schedule.add",
            json!({"when": {"at": miyu_kernel::time::Timestamp::from_unix_millis(at).unwrap()},
                   "prompt": "早报", "target": {"new": {"cwd": cwd}}}),
        )
        .await;
    assert_eq!(added["result"]["schedule"]["id"], "t2", "{added}");
    hand.advance(30_000);
    let told = notice(&mut client).await;
    assert_eq!(told["kind"], "schedule.ran", "已有会话那一次不推：{told}");
    assert_eq!(told["schedule"], "t2");
    assert_eq!(told["ok"], true);
    let opened = told["session"].as_str().expect("带会话");
    assert_ne!(opened, session, "是新开的那一个");
    home.until_turns(opened, 1).await;
    assert!(told["at"].is_string(), "{told}");
}

#[tokio::test]
async fn a_broken_table_is_told_when_it_is_replaced() {
    let home = Home::new();
    let hand = Hand::at(START);
    let core = core_with(&home, &Script::new([]), &hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let table = home.root.path().join("state").join("schedules.db");
    std::fs::create_dir_all(table.parent().expect("有上级")).expect("建得了");
    std::fs::write(&table, "not a database").expect("写得进");
    let subscribed = client
        .call("n", "subscribe", json!({"stream": "notices"}))
        .await;
    assert_eq!(subscribed["result"], json!({}), "{subscribed}");
    let listed = client.call("l", "schedule.list", json!({})).await;
    assert_eq!(
        listed["result"]["schedules"],
        json!([]),
        "换了一份空的：{listed}"
    );
    let told = notice(&mut client).await;
    assert_eq!(told["kind"], "schedules.reset", "{told}");
}
