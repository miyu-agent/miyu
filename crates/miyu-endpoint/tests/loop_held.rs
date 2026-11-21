//! 有能恢复的撤销时敲的 `/loop`（施工 FX-12）：真核心、手拨的钟。这时循环不开一轮，回执照实说循环先搁着、发一句话或
//! `/restore` 以后开始；没有撤销的照原来的说。以前回执照常说「已开始循环」，人等着却什么都没发生。
//!
//! 搁着的循环不交那一句（施工 FX-13）：同会话正忙时一样只在内存里等（`busy.rs`），撤回处理了才交、由它自己开一轮。以前交进去
//! 只记下，日志里多一条没人回答的 `message.user`，`/loop off` 以后 `/restore` 还由它开一轮、发一句话时她也看得到。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 循环这个包的工具。
fn looping() -> Catalog {
    let tools = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    Catalog::in_packages([(miyu_schedule::LOOP, tools)]).expect("合写法")
}

/// 唤醒表交进来的那几条消息。
fn from_schedule(log: &[Event]) -> Vec<&Event> {
    log.iter()
        .filter(|event| matches!(event.by, By::Schedule(_)))
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .collect()
}

/// 敲一条命令，交回回执。
async fn typed(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    let params = json!({"session": session, "text": text});
    client.call(id, "command.run", params).await
}

#[tokio::test]
async fn a_loop_typed_while_a_revert_is_pending_says_it_waits() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("看过了。"),
        Play::Says("又看了。"),
    ]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let reverted = client
        .call("r1", "session.revert", json!({"session": session}))
        .await;
    assert!(reverted.get("error").is_none(), "{reverted}");
    let reply = typed(&mut client, "k1", &session, "/loop 5m 看看 CI").await;
    assert_eq!(
        reply["result"]["said"],
        "已开始循环：每 5m · 看看 CI。有一次撤回还没有处理，循环暂不运行；发送一条消息或运行 /restore 后开始",
        "{reply}"
    );
    // 撤销还在：循环不开一轮，那一句也不交（施工 FX-13）。
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(script.requests().len(), 1, "撤销还在，循环不开一轮");
    assert!(
        from_schedule(&home.log(&session)).is_empty(),
        "搁着的不留话"
    );
    // 恢复以后照常开一轮。
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert!(restored.get("error").is_none(), "{restored}");
    home.until_turns(&session, 2).await;
    assert_eq!(
        from_schedule(&home.log(&session)).len(),
        1,
        "恢复以后交一句"
    );
    let reply = typed(&mut client, "k2", &session, "/loop 5m 看看 CI").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：每 5m · 看看 CI",
        "没有撤销的照原来的说"
    );
}

/// 起好的一份：说过一轮、撤掉了，撤销还没处理时敲了两次 `/loop 10m 看一眼时间`，再 `/loop off`。
async fn held_then_off(home: &Home, script: &Script) -> (Client, String) {
    let hand = Hand::at(START);
    let core = core_with_tools(home, script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let reverted = client
        .call("r1", "session.revert", json!({"session": session}))
        .await;
    assert!(reverted.get("error").is_none(), "{reverted}");
    typed(&mut client, "k1", &session, "/loop 10m 看一眼时间").await;
    typed(&mut client, "k2", &session, "/loop 10m 看一眼时间").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        from_schedule(&home.log(&session)).is_empty(),
        "搁着的循环不留话：{:?}",
        home.log(&session)
    );
    let off = typed(&mut client, "k3", &session, "/loop off").await;
    assert_eq!(off["result"]["said"], "已停止这个会话的循环。", "{off}");
    (client, session)
}

/// 搁着时停掉的循环，恢复撤销以后不由它开一轮（施工 FX-13）：以前那两句记在日志里、记在内核一边，恢复时由最后那句开一轮。
#[tokio::test]
async fn a_loop_held_then_turned_off_does_not_run_after_restore() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("不该有这一轮。")]);
    let (mut client, session) = held_then_off(&home, &script).await;
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert!(restored.get("error").is_none(), "{restored}");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(script.requests().len(), 1, "停掉的循环不开轮");
}

/// 搁着时停掉的循环，人再说一句：她看不到那两句陈旧的话（施工 FX-13）。
#[tokio::test]
async fn a_loop_held_then_turned_off_leaves_nothing_for_the_next_message() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("在的。")]);
    let (mut client, session) = held_then_off(&home, &script).await;
    client.say("s2", &session, "还在吗").await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    let last = serde_json::to_string(&requests.last().expect("发了").1.messages).expect("JSON");
    assert!(!last.contains("看一眼时间"), "{last}");
}
