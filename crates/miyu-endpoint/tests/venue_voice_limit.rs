//! 找回的场所会话也认语音的上限（施工 VM-2，`docs/blueprint/venues.md`「`venue.session`」）：造的时候桥没报 `voice_max_ms`
//! 的会话，找回时报了，记一条带它的 `session.policy_changed`（`by` 是桥），之后的 `tool.call` 带上；再报一样的、不报的什么都
//! 不记，照记过的。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};

use crate::support::extensions::*;
use crate::support::served::{replies, served_core};
use crate::support::*;

/// 桥登记的一件：哪个场所都给。
fn tools() -> Value {
    json!({"tools": [{"name": "note", "description": "A test tool.", "input_schema": {"type": "object"},
                      "access": "read", "venues": ["local", "group", "private"]}]})
}

/// 等到扩展记下核心发来的 `tool.call`，交回那一条。
async fn tool_call(path: &std::path::Path) -> Value {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let found = read(path)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|line| line["method"] == "tool.call");
        if let Some(call) = found {
            return call;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到 tool.call");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_found_venue_session_takes_the_voice_limit_reported_now() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    let group = r#"{"venue":"qq:group:5","kind":"group"}"#;
    let limited = r#"{"venue":"qq:group:5","kind":"group","voice_max_ms":60000}"#;
    install_serving(
        &home,
        "bridge",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{}", tools()),
            &format!("ask:venue.session:{group}"),
            &format!("ask:venue.session:{limited}"),
            // 再报一样的、不报的：照记过的，什么都不记。
            &format!("ask:venue.session:{limited}"),
            &format!("ask:venue.session:{group}"),
            r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:10003"}}"#,
            "serve",
        ]),
        "always",
        true,
    );
    let script = Script::new([Play::calls(&[("note", "{}")]), Play::Says("好。")]);
    let _core = served_core(&home, &script);
    let got = replies(&path, 7).await;
    let session = got[2]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{got:?}"))
        .to_string();
    assert_eq!(got[2]["result"]["created"], true, "{got:?}");
    for found in &got[3..6] {
        assert_eq!(
            found["result"],
            json!({"session": session, "created": false}),
            "{got:?}"
        );
    }
    let call = tool_call(&path).await;
    assert_eq!(call["params"]["voice_max_ms"], json!(60_000), "{call}");
    let log = home.log(&session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条是造会话");
    };
    assert_eq!(created.voice_max_ms, None, "造的时候没报");
    let changed: Vec<_> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::PolicyChanged(changed) => changed.voice_max_ms.map(|ms| (ms, &event.by)),
            _ => None,
        })
        .collect();
    assert_eq!(changed.len(), 1, "只记一次：{changed:?}");
    assert_eq!(changed[0].0, 60_000);
    assert!(
        matches!(changed[0].1, By::Module(module) if module.id.as_str() == "bridge"),
        "记成报的那个桥：{changed:?}"
    );
}
