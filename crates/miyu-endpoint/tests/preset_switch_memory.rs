//! 中途换到没开记忆的预设（施工 FX-8）：项目主人的群会话 2026-11-20 换到「无功能」以后，技能关了，记忆却照样每轮注入常驻的
//! 摘要、名片、联想，记忆的三件工具也还在。换了以后的那一轮起：回合开始不再问记忆，工具面上没有记忆的工具。直接用「无功能」
//! 开的会话一开始就没有。换回开着记忆的预设记忆回来，由 `miyu-policy` 的 `keeping_memory` 的单元测试守着。
//!
//! 常驻的摘要只在这一段上下文里没有时注入：换预设时单开的那一轮压缩把它压掉了，没修的时候下一轮又注入一次。

use serde_json::json;

use miyu_kernel::event::Body;

use crate::preset_face::tool_names;
use crate::support::*;
use miyu_session::testkit::{Play, Script};

/// 记一条记忆 `id`，常驻的摘要里就有它（`memory.rs` 的 `a_new_session_starts_with_what_was_remembered`）。
async fn remembered(client: &mut Client, id: &str, text: &str) {
    let reply = client
        .call(
            id,
            "memory.remember",
            json!({"class": "user", "text": text}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": id}), "{reply}");
}

/// 会话日志里换预设那一条以后，记忆注入了几块（常驻的摘要、名片、联想都是记忆这个模块注入的）。
fn memory_after_switch(home: &Home, session: &str) -> usize {
    let log = home.log(session);
    let switched = log
        .iter()
        .position(|event| matches!(&event.body, Body::PolicyChanged(_)))
        .expect("换过预设");
    log[switched..]
        .iter()
        .filter(|event| {
            matches!(&event.body, Body::ContextInjected(fact)
                if ["memory", "card", "recall"].contains(&fact.kind.as_str()))
        })
        .count()
}

/// 带人格、带记忆三件假工具的核心（`preset_face.rs` 的工具目录），握好手。
async fn connected(home: &Home, script: &Script) -> Client {
    let core = crate::support::memories::with_persona(home, script, crate::preset_face::catalog());
    let mut client = Client::connect(core);
    client.hello().await;
    client
}

/// 换预设，回应不是错的。
async fn switch(client: &mut Client, id: &str, session: &str, preset: &str) {
    let reply = client
        .call(
            id,
            "session.configure",
            json!({"session": session, "preset": preset}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
}

#[tokio::test]
async fn switching_to_a_preset_without_memory_stops_memory_from_the_next_turn() {
    let home = Home::new();
    // 第一句说七万个字：换预设时单开的那一轮压缩压得掉它（`compact.rs`）。压缩以后常驻的摘要要重新注入，项目主人的会话就是
    // 这样在换了以后又注入了一次。
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
        Play::Says("好。"),
    ])
    .window(1_000_000);
    let mut client = connected(&home, &script).await;
    remembered(&mut client, "m1", "用户养了一只猫").await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client
        .create_with(
            "c1",
            json!({"cwd": work, "persona": "engineer", "preset": "dev"}),
        )
        .await;
    client.say("s1", &session, &"x".repeat(70_000)).await;
    home.until_turns(&session, 1).await;
    let first = script.requests().last().expect("问过模型").1.clone();
    assert!(
        format!("{first:?}").contains("<memories>"),
        "开着记忆的常驻摘要"
    );
    assert!(tool_names(&first).contains(&"remember"));

    switch(&mut client, "k1", &session, "none").await;
    home.until_turns(&session, 2).await;
    assert!(
        home.log(&session)
            .iter()
            .any(|event| matches!(&event.body, Body::ContextCompacted(_))),
        "换预设时压了一次"
    );
    client.say("s2", &session, "还在吗").await;
    home.until_turns(&session, 3).await;
    let after = script.requests().last().expect("问过模型").1.clone();
    assert!(
        !tool_names(&after).contains(&"remember"),
        "无功能里没有记忆的工具：{:?}",
        tool_names(&after)
    );
    assert_eq!(memory_after_switch(&home, &session), 0, "换了以后不再注入");
    assert!(
        !format!("{after:?}").contains("<memories>"),
        "压掉以后不再带"
    );
}

#[tokio::test]
async fn a_session_opened_with_no_features_has_no_memory_at_all() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = connected(&home, &script).await;
    remembered(&mut client, "m1", "用户养了一只猫").await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client
        .create_with(
            "c1",
            json!({"cwd": work, "persona": "engineer", "preset": "none"}),
        )
        .await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let request = script.requests().last().expect("问过模型").1.clone();
    assert!(
        tool_names(&request).is_empty(),
        "{:?}",
        tool_names(&request)
    );
    assert!(!format!("{request:?}").contains("<memories>"));
}
