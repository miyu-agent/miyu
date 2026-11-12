//! 内存不够不开会话（施工 MEM-1，`docs/blueprint/protocol.md`「会话表」第 10 条）：读数照测试给的（`Core::with_memory_probe`），
//! 不真把内存吃满。只剩 3% 时新造会话、从磁盘载入会话、分叉都拒 `low_memory`，带剩多少、说给人听的那一句照剩多少说，什么都不造；
//! 已经开着的会话照常说话；同一个造会话的编号重发照旧交回原来那一个；读不到（不知道）的照常开。

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_endpoint::Core;
use miyu_heap::room::Room;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

const GIB: u64 = 1024 * 1024 * 1024;

/// 只剩 3%：100 GiB 里剩 3 GiB。
const LOW: Room = Room {
    available: 3 * GIB,
    total: 100 * GIB,
};

/// 宽裕：一半。
const PLENTY: Room = Room {
    available: 5 * GIB,
    total: 10 * GIB,
};

/// 测试中途改得了的读数：没有的是不知道。
type Reading = Arc<Mutex<Option<Room>>>;

fn reading(room: Option<Room>) -> Reading {
    Arc::new(Mutex::new(room))
}

fn set(reading: &Reading, room: Option<Room>) {
    *reading.lock().unwrap_or_else(PoisonError::into_inner) = room;
}

/// 一份核心，还剩多少内存照 `reading`。
fn core(home: &Home, script: &Script, reading: &Reading) -> Arc<Core> {
    let reading = Arc::clone(reading);
    Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_sandbox(miyu_sandbox::Availability::Usable("miyu-sandbox".into()))
            .with_memory_probe(move || *reading.lock().unwrap_or_else(PoisonError::into_inner)),
    )
}

/// 拒的是 `low_memory`，带着剩多少，给人看的那一句照剩多少说（握手是中文）。
fn low_memory(reply: &serde_json::Value) {
    assert_eq!(reason(reply), Some("low_memory"), "{reply}");
    assert_eq!(reply["error"]["code"], json!(-32010), "{reply}");
    assert_eq!(
        reply["error"]["data"]["available_mib"],
        json!(3072),
        "{reply}"
    );
    assert_eq!(
        reply["error"]["data"]["available_percent"],
        json!(3),
        "{reply}"
    );
    assert_eq!(
        reply["error"]["message"],
        json!("可用内存只剩 3072 MB（3%），暂时不能打开会话。请关掉一些程序后重试。")
    );
}

#[tokio::test]
async fn low_memory_refuses_a_new_session_but_open_ones_go_on() {
    let home = Home::new();
    let reading = reading(Some(PLENTY));
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(core(&home, &script, &reading));
    client.hello().await;
    let open = client.create("c1", "~").await;
    set(&reading, Some(LOW));
    let refused = client
        .call("c2", "session.create", json!({"cwd": "~"}))
        .await;
    low_memory(&refused);
    assert_eq!(home.root.sessions().expect("列得出").len(), 1, "没造第二个");
    // 重发造过的那一个：不是新开，照旧交回。
    assert_eq!(client.create("c1", "~").await, open);
    // 开着的照常说话。
    let said = client.say("s1", &open, "你好").await;
    assert!(said.get("result").is_some(), "{said}");
    home.until_turns(&open, 1).await;
}

#[tokio::test]
async fn low_memory_refuses_loading_a_session_from_disk() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let session = {
        let mut client = Client::connect(core(&home, &script, &reading(Some(PLENTY))));
        client.hello().await;
        client.create("c1", "~").await
    };
    let before = home.log(&session).len();
    // 重启的核心：会话不在表里，要从磁盘载入。
    let reading = reading(Some(LOW));
    let mut client = Client::connect(core(&home, &script, &reading));
    client.hello().await;
    low_memory(&client.say("s1", &session, "你好").await);
    assert_eq!(home.log(&session).len(), before, "日志一条没多");
    // 不知道的照常载入、照常说话。
    set(&reading, None);
    let said = client.say("s2", &session, "你好").await;
    assert!(said.get("result").is_some(), "{said}");
    home.until_turns(&session, 1).await;
}

#[tokio::test]
async fn low_memory_refuses_a_fork_without_leaving_one_on_disk() {
    let home = Home::new();
    let reading = reading(Some(PLENTY));
    let mut client = Client::connect(core(&home, &Script::new([]), &reading));
    client.hello().await;
    let session = client.create("c1", "~").await;
    set(&reading, Some(LOW));
    let refused = client
        .call("f1", "session.fork", json!({"session": session}))
        .await;
    low_memory(&refused);
    assert_eq!(home.root.sessions().expect("列得出").len(), 1, "没留下支线");
}

#[tokio::test]
async fn unknown_memory_opens_sessions_as_usual() {
    let home = Home::new();
    let mut client = Client::connect(core(&home, &Script::new([]), &reading(None)));
    client.hello().await;
    client.create("c1", "~").await;
    client.create("c2", "~").await;
    assert_eq!(home.root.sessions().expect("列得出").len(), 2);
}
