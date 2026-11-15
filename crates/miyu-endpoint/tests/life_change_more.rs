//! 后天传到别的会话、跟着人生结束清掉（施工 LF-11，设计 `70-推演人生.md` 第六节「定了的」第 2、4 条）：真核心、手拨的钟。同一个
//! 人格另开的会话第一轮的常驻那一块里有人生会话记下的变化，别的人格的没有；主人 `life.kill`、她自己 `end_life` 以后那几条清掉
//! 了，别的会话的常驻那一块里没有；`life.new` 开的新一段从零条开始。

use std::time::Duration;

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};

use crate::life_change::{changes, note, request, start, started};
use crate::support::clock::Hand;
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// `software/memory/summary/changes.txt`。
const TITLE: &str = "How I have changed over this life, noted by me:";

/// 请求里常驻那一块；没有的没有。
fn summary(request: &Request) -> Option<String> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.clone(),
            _ => Vec::new(),
        })
        .find_map(|block| match block {
            Block::Text(Text { text }) if text.starts_with("<memories>") => Some(text),
            _ => None,
        })
}

/// 人格 `persona` 另开一个普通会话说一句：那一轮常驻那一块（没有的没有）。`n` 是这是第几次带工具面的请求。
async fn elsewhere(
    client: &mut Client,
    home: &Home,
    script: &Script,
    persona: &str,
    n: usize,
) -> Option<String> {
    let session = client
        .create_with(&format!("c{n}"), json!({"cwd": "~", "persona": persona}))
        .await;
    client.say(&format!("s{n}"), &session, "在吗").await;
    home.until_turns(&session, 1).await;
    summary(&request(script, n))
}

/// 等 `muse` 那一间类是 `change` 的清空（最多 10 秒）：结束以后的清理不等那一轮。
async fn until_cleared(client: &mut Client) {
    for n in 0..200 {
        if changes(client, &format!("w{n}")).await.is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("十秒没清掉");
}

/// 验收第 6 条：同一个人格另开的会话第一轮的常驻那一块里有她记下的变化（标题、那一行），别的人格的会话没有。
#[tokio::test]
async fn another_session_of_hers_carries_the_change_and_others_do_not() {
    let home = Home::new();
    let script = Script::new([
        note(json!({"text": "以前爱喝咖啡，这个月改喝茶"})),
        Play::Says("记下了。"),
        Play::Says("在。"),
        Play::Says("在。"),
    ]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    let hers = elsewhere(&mut client, &home, &script, "muse", 2)
        .await
        .expect("交了常驻那一块");
    assert!(
        hers.starts_with(&format!("<memories>\n{TITLE}\nm1 change ")),
        "{hers}"
    );
    assert!(hers.contains(": 以前爱喝咖啡，这个月改喝茶\n"), "{hers}");
    let other = elsewhere(&mut client, &home, &script, "sage", 3).await;
    assert!(
        other.as_deref().is_none_or(|block| !block.contains(TITLE)),
        "别的人格没有：{other:?}"
    );
}

/// 验收第 9 条：主人 `life.kill` 以后那几条清掉了，别的会话的常驻那一块里没有；`life.new` 开的新一段从零条开始。
#[tokio::test]
async fn killing_the_life_clears_its_changes_and_a_new_life_starts_from_none() {
    let home = Home::new();
    let script = Script::new([
        note(json!({"text": "以前爱喝咖啡，这个月改喝茶"})),
        Play::Says("记下了。"),
        Play::Says("在。"),
        Play::Says("又醒了。"),
    ]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    assert_eq!(changes(&mut client, "c0").await.len(), 1);
    let killed = client
        .call("k", "life.kill", json!({"persona": "muse"}))
        .await;
    assert_eq!(killed["result"]["state"], "ended", "{killed}");
    until_cleared(&mut client).await;
    let block = elsewhere(&mut client, &home, &script, "muse", 2).await;
    assert!(
        block.as_deref().is_none_or(|block| !block.contains(TITLE)),
        "{block:?}"
    );
    let renewed = client
        .call("n", "life.new", json!({"persona": "muse"}))
        .await;
    let fresh = renewed["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{renewed}"))
        .to_string();
    home.until_turns(&fresh, 1).await;
    assert!(
        changes(&mut client, "c1").await.is_empty(),
        "新的一段从零条开始"
    );
    let first = summary(&request(&script, 3));
    assert!(
        first.as_deref().is_none_or(|block| !block.contains(TITLE)),
        "{first:?}"
    );
}

/// 验收第 9 条：她自己 `end_life` 以后同样清掉。
#[tokio::test]
async fn her_own_end_clears_her_changes() {
    let home = Home::new();
    let script = Script::new([
        note(json!({"text": "以前爱喝咖啡，这个月改喝茶"})),
        Play::calls(&[("end_life", r#"{"last_words":"谢谢。"}"#)]),
        Play::Says("我走了。"),
        Play::Says("在。"),
    ]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    until_cleared(&mut client).await;
    let block = elsewhere(&mut client, &home, &script, "muse", 3).await;
    assert!(
        block.as_deref().is_none_or(|block| !block.contains(TITLE)),
        "{block:?}"
    );
}
