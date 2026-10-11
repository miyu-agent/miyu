//! 想起过什么（施工 R-4 下，`docs/blueprint/memory.md` 第八条）：真会话、真记忆日志、真回合库。常驻那一块带过的、她
//! `memory_search` 搜到的，各算用到一次，最近一次是那一条事件的时刻；跟着回合索引写进这一间的回合库。

use std::collections::BTreeMap;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::session::Command;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Used;
use miyu_tool::Catalog;

use crate::support::meaning::{persona, save};
use crate::support::*;

fn catalog(home: &Home) -> Catalog {
    let mut tools = miyu_basesystem::tools(home.resources.path()).expect("读得出");
    tools.extend(miyu_memory::tools(home.resources.path()).expect("读得出"));
    Catalog::new(tools).expect("合写法")
}

/// 说一句，等这一轮结束；再停下：日志先落盘、回合库后写，停下要等手上那一批写完。交回日志。
async fn chat_and_stop(home: &Home, handle: &Handle, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within("回应", handle.command(id("cmd-1"), alice(), said))
        .await
        .expect("会话在跑");
    let log = until_logged(home, handle.id(), |log| {
        log.iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_)))
    })
    .await;
    stop(handle).await;
    log
}

fn uses(home: &Home) -> BTreeMap<String, Used> {
    home.recall.turns(&persona()).0.uses().expect("读得了")
}

#[tokio::test]
async fn what_the_block_brought_and_what_she_found_are_counted() {
    let home = Home::new();
    save(&home, "用户养了一只猫");
    let script = Script::new([Play::Says("好。")]);
    let first = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat_and_stop(&home, &first, "在吗").await;
    assert_eq!(
        uses(&home).get("m1").map(|used| used.count),
        Some(1),
        "常驻那一块带过一次"
    );

    let script = Script::new([
        Play::calls(&[("memory_search", r#"{"query":"猫"}"#)]),
        Play::Says("你养了一只猫。"),
    ]);
    let second = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let log = chat_and_stop(&home, &second, "我养了什么").await;
    let found = log
        .iter()
        .find(|event| matches!(event.body, Body::ToolResult(_)))
        .expect("搜了一次");
    assert_eq!(
        uses(&home).get("m1"),
        Some(&Used {
            count: 3,
            last: found.at
        }),
        "第二个会话常驻那一块一次、她搜到一次，最近一次是搜到的那一刻"
    );
}

/// 回合库里落下了（更新失败、崩在更新之前）：载入时照日志补回来，和回合索引一起（第八条第 2 款）。
#[tokio::test]
async fn loading_catches_up_what_was_missed() {
    let home = Home::new();
    save(&home, "用户养了一只猫");
    let script = Script::new([Play::Says("好。")]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let id = handle.id().clone();
    chat_and_stop(&home, &handle, "在吗").await;
    home.recall
        .turns(&persona())
        .0
        .forget(id.as_str())
        .expect("拿得掉");
    assert!(uses(&home).is_empty(), "删会话时跟着拿掉");
    let handle = home.load(&id, &Script::new([])).await;
    stop(&handle).await;
    assert_eq!(uses(&home).get("m1").map(|used| used.count), Some(1));
}
