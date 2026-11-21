//! 撤回和循环（施工 FX-12、FX-13、FX-14）：真核心、手拨的钟。
//!
//! 有能恢复的撤销时敲的 `/loop` 算人开口（施工 FX-14，项目主人 2026-11-22 定）：和人发一句话一样接受了这次撤回，回执照常说
//! 「已开始循环」，第一次马上开一轮，撤掉的那几轮从此不能 `/restore`。FX-12 时回执说循环先搁着、等人发话或恢复。
//!
//! 循环早就建好、之后人才撤回、然后到点：人没有新操作，照旧搁着（施工 FX-13）：同会话正忙时一样只在内存里等（`busy.rs`），
//! 撤回处理了才交、由它自己开一轮。以前交进去只记下，日志里多一条没人回答的 `message.user`，`/loop off` 以后 `/restore`
//! 还由它开一轮、发一句话时她也看得到。

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

/// 起好的一份：说过一轮、撤掉了，撤销还没处理。
async fn reverted(home: &Home, script: &Script) -> (Client, String) {
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
    (client, session)
}

/// 撤销没处理时敲 `/loop <间隔> <话>`：回执照常说，马上开一轮，之后恢复不了（施工 FX-14）。
#[tokio::test]
async fn a_loop_typed_while_a_revert_is_pending_starts_right_away() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("看过了。")]);
    let (mut client, session) = reverted(&home, &script).await;
    let reply = typed(&mut client, "k1", &session, "/loop 5m 看看 CI").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：每 5m · 看看 CI",
        "{reply}"
    );
    home.until_turns(&session, 2).await;
    assert_eq!(from_schedule(&home.log(&session)).len(), 1, "交了一句");
    let requests = script.requests();
    assert_eq!(requests.len(), 2, "循环的第一次开了一轮");
    let last = serde_json::to_string(&requests.last().expect("发了").1.messages).expect("JSON");
    assert!(last.contains("看看 CI"), "{last}");
    assert!(!last.contains("在吗"), "撤掉的那一轮她看不到：{last}");
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert_eq!(reason(&restored), Some("nothing_to_unrevert"), "{restored}");
}

/// 撤销没处理时光打 `/loop`：同上，回执是自主循环的那一句（施工 FX-14）。
#[tokio::test]
async fn an_autonomous_loop_typed_while_a_revert_is_pending_starts_right_away() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("接着做了。")]);
    let (mut client, session) = reverted(&home, &script).await;
    let reply = typed(&mut client, "k1", &session, "/loop").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：由 AI 决定执行的内容和下一次运行的时间",
        "{reply}"
    );
    home.until_turns(&session, 2).await;
    assert_eq!(script.requests().len(), 2, "循环的第一次开了一轮");
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert_eq!(reason(&restored), Some("nothing_to_unrevert"), "{restored}");
}

/// 起好的一份：没有撤销时敲 `/loop 10m 看一眼时间`、跑了第一次，人撤掉这一轮，到了第二次（施工 FX-13）。
async fn undone_then_due(home: &Home, script: &Script) -> (Client, String) {
    let hand = Hand::at(START);
    let core = core_with_tools(home, script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    let reply = typed(&mut client, "k1", &session, "/loop 10m 看一眼时间").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：每 10m · 看一眼时间",
        "没有撤销的照原来的说：{reply}"
    );
    home.until_turns(&session, 1).await;
    let reverted = client
        .call("r1", "session.revert", json!({"session": session}))
        .await;
    assert!(reverted.get("error").is_none(), "{reverted}");
    hand.advance(600_000);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(script.requests().len(), 1, "撤销还在，到点的不开一轮");
    assert_eq!(
        from_schedule(&home.log(&session)).len(),
        1,
        "搁着的不留话：{:?}",
        home.log(&session)
    );
    (client, session)
}

/// 先建循环、再撤回、然后到点：搁着，恢复以后照常开一轮（施工 FX-13）。
#[tokio::test]
async fn a_loop_due_while_a_revert_is_pending_waits_for_restore() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("又看了。")]);
    let (mut client, session) = undone_then_due(&home, &script).await;
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert!(restored.get("error").is_none(), "{restored}");
    home.until_turns(&session, 2).await;
    assert_eq!(
        from_schedule(&home.log(&session)).len(),
        2,
        "恢复以后交一句"
    );
}

/// 搁着时停掉的循环，恢复撤销以后不由它开一轮（施工 FX-13）。
#[tokio::test]
async fn a_loop_held_then_turned_off_does_not_run_after_restore() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("不该有这一轮。")]);
    let (mut client, session) = undone_then_due(&home, &script).await;
    let off = typed(&mut client, "k2", &session, "/loop off").await;
    assert_eq!(off["result"]["said"], "已停止这个会话的循环。", "{off}");
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert!(restored.get("error").is_none(), "{restored}");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(script.requests().len(), 1, "停掉的循环不开轮");
}

/// 搁着时停掉的循环，人再说一句：她看不到那句陈旧的话（施工 FX-13）。
#[tokio::test]
async fn a_loop_held_then_turned_off_leaves_nothing_for_the_next_message() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("在的。")]);
    let (mut client, session) = undone_then_due(&home, &script).await;
    let off = typed(&mut client, "k2", &session, "/loop off").await;
    assert_eq!(off["result"]["said"], "已停止这个会话的循环。", "{off}");
    client.say("s2", &session, "还在吗").await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    let last = serde_json::to_string(&requests.last().expect("发了").1.messages).expect("JSON");
    // 第一次交进去的那句撤销不带走（不是人说的话，`kernel/session.md`「定时交进来的话」）；搁着的那一次没交，不多一句。
    assert_eq!(last.matches("看一眼时间").count(), 1, "{last}");
}
