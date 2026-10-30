//! 发布前测试（2026-10-30）安全那一份的防御性测试（只断言「不该发生的没发生」，修之前是红的）。施工 AS-3：`schedule.add` 的
//! `target.session` 原来不过「会话只给碰得到它的」那一道（`connection/checks.rs` 只看参数顶上的 `session`）：平台桥碰不到本机的
//! 会话、人的头碰不到关着的群会话，都不该能往里面排定时的话。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::extensions::*;
use crate::support::served::{replies, served_core};
use crate::support::*;

/// 人的头看到的全部定时任务。
async fn schedules(client: &mut Client) -> Vec<Value> {
    let listed = client.call("l", "schedule.list", json!({})).await;
    listed["result"]["schedules"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .clone()
}

#[tokio::test]
async fn audit_a_bridge_cannot_schedule_into_a_session_it_cannot_reach() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let local_session = home.work.join("local-session");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("session:{}", local_session.display()),
            r#"ask:schedule.add:{"when":{"every":3600},"prompt":"from the bridge","target":{"session":"{session}"}}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let core = served_core(&home, &Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let here = client.create("c1", "~").await;
    std::fs::write(&local_session, &here).expect("写得进");
    let got = replies(&path, 2).await;
    assert!(
        got[1].get("error").is_some(),
        "桥碰不到本机的会话，不该能往里排定时的话：{got:?}"
    );
    let listed = schedules(&mut client).await;
    assert!(
        listed
            .iter()
            .all(|one| one["target"]["session"] != json!(here)),
        "唤醒表里不该有桥排进本机会话的：{listed:?}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn audit_a_head_cannot_schedule_into_a_confined_group() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let group = r#"{"venue":"qq:group:1","kind":"group","persona":"engineer"}"#;
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("ask:venue.session:{group}"),
            "wait",
        ]),
        "always",
        true,
    );
    let core = served_core(&home, &Script::new([]));
    let got = replies(&path, 2).await;
    let session = got[1]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{got:?}"))
        .to_string();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    // 同一道检查：头订阅关着的群会话是 `session_not_found`。
    let seen = client.subscribe("g1", &session).await;
    assert_eq!(reason(&seen), Some("session_not_found"), "{seen}");
    let added = client
        .call(
            "a1",
            "schedule.add",
            json!({"when": {"every": 3600}, "prompt": "from the head", "target": {"session": session}}),
        )
        .await;
    assert!(
        added.get("error").is_some(),
        "头碰不到关着的群会话，不该能往里排定时的话：{added}"
    );
    let listed = schedules(&mut client).await;
    assert!(
        listed
            .iter()
            .all(|one| one["target"]["session"] != json!(session)),
        "唤醒表里不该有排进关着的群会话的：{listed:?}"
    );
    core.stop_extensions().await;
}
