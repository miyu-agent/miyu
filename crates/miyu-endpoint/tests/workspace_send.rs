//! 说话带的工作目录（施工 9-7 再补，终端无缝集成：每一轮的工作目录跟着 shell 走，设计 22 O6）：`session.send` 带 `cwd`，和现在
//! 一样的什么都不记；不一样的先换（一条 `session.workspace_changed`），这一句开的那一轮照新目录跑；写错的当场拒绝、什么都不送；
//! 代表平台上的人说话的不收。

use serde_json::json;

use crate::support::*;
use miyu_kernel::event::{Body, Event};
use miyu_session::testkit::{Play, Script};

/// 工作区旁边建一个目录，交回它的路径（数据根外面）。
fn dir(home: &Home, name: &str) -> String {
    let dir = home.work.join(name);
    std::fs::create_dir_all(&dir).expect("建得了目录");
    dir.to_string_lossy().into_owned()
}

/// 日志里换工作目录的几条：换到哪。
fn moves(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::WorkspaceChanged(changed) => Some(changed.cwd.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_send_moves_the_workspace_only_when_it_changed() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("好。"), Play::Says("好。")]);
    let start = dir(&home, "start");
    let other = dir(&home, "other");
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", &start).await;

    let same = client
        .call(
            "s1",
            "session.send",
            json!({"session": session, "text": "在这儿", "cwd": start}),
        )
        .await;
    assert_eq!(same["result"]["cwd"], json!(start), "{same}");
    home.until_turns(&session, 1).await;
    assert!(moves(&home.log(&session)).is_empty(), "一样的什么都不记");

    let moved = client
        .call(
            "s2",
            "session.send",
            json!({"session": session, "text": "换个地方", "cwd": other}),
        )
        .await;
    assert_eq!(moved["result"]["cwd"], json!(other), "{moved}");
    home.until_turns(&session, 2).await;
    let log = home.log(&session);
    assert_eq!(moves(&log), std::slice::from_ref(&other));
    let changed = log
        .iter()
        .position(|event| matches!(event.body, Body::WorkspaceChanged(_)))
        .expect("换了");
    let said = log
        .iter()
        .rposition(|event| matches!(event.body, Body::MessageUser(_)))
        .expect("说了");
    assert!(changed < said, "先换目录再说");

    // 写错的当场拒绝：什么都不送。
    let before = home.log(&session).len();
    let missing = home.work.join("no-such-dir").to_string_lossy().into_owned();
    let refused = client
        .call(
            "s3",
            "session.send",
            json!({"session": session, "text": "去哪", "cwd": missing}),
        )
        .await;
    assert_eq!(reason(&refused), Some("path_unreadable"), "{refused}");
    assert_eq!(home.log(&session).len(), before, "{refused}");

    // 代表平台上的人说话的不收。
    let venue = client
        .call(
            "s4",
            "session.send",
            json!({"session": session, "text": "hi", "cwd": other, "as": {"external": "qq:1"}}),
        )
        .await;
    assert_eq!(reason(&venue), Some("bad_params"), "{venue}");
}
