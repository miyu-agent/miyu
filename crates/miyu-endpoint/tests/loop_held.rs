//! 有能恢复的撤销时敲的 `/loop`（施工 FX-12）：真核心、手拨的钟。这时循环交来的那一句只记下、不开一轮（内核 `jobs.rs` 的
//! `can_wake`，这条规矩不动），回执照实说循环先搁着、发一句话或 `/restore` 以后开始；没有撤销的照原来的说。以前回执照常说
//! 「已开始循环」，人等着却什么都没发生。

use std::sync::Arc;

use serde_json::{Value, json};

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
    // 规矩不动：循环交来的那一句记下了，不开一轮。
    assert_eq!(script.requests().len(), 1, "撤销还在，循环不开一轮");
    // 恢复以后照常开一轮。
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert!(restored.get("error").is_none(), "{restored}");
    home.until_turns(&session, 2).await;
    let reply = typed(&mut client, "k2", &session, "/loop 5m 看看 CI").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：每 5m · 看看 CI",
        "没有撤销的照原来的说"
    );
}
