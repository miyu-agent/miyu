//! 后台整理什么时候合（施工 E-9，`docs/blueprint/memory.md` 第七条第 1 款）：只看离上次合并多久、不看抽过几个会话；只在这一次
//! 抽取真记下了东西时看，抽到 0 条的不看，没有轮询。真会话、真记忆日志；整理记忆的模型是一台假服务器（`org/m`）。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::id::Seq;
use miyu_kernel::time::Timestamp;
use miyu_recall::{MemoryEvent, Merged};

use crate::memory_merge::{merged, session, spare, until};
use crate::support::extracting::*;
use crate::support::meaning::{chat, persona, save};
use crate::support::*;

/// 一直只在一个会话里聊（施工 E-9：QQ 主群那样，抽取几乎都来自同一个会话）：离上次合并满了 `memory.merge_every` 就合，不用
/// 凑够几个会话；合过的时间没到不再合。
#[tokio::test]
async fn one_busy_session_is_merged_once_the_interval_has_passed() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        merged(serde_json::json!({"summary": "用户养猫。"})),
        every_turn("用户住在东京"),
        spare(),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    // 很久以前合过一次：间隔早过了。
    let long_ago = Timestamp::parse("2026-01-01T00:00:00.000Z").expect("合写法");
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let mark = Merged {
        upto: Seq::new(1).expect("不是零"),
        given: 0,
        revised: 0,
        retired: 0,
        failed: false,
    };
    log.append(long_ago, alice(), None, &MemoryEvent::Merged(mark))
        .expect("记得下");
    let handle = session(&home, 4).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "橘猫").await;
    requested(&server, 2).await;
    until(&home, |book| {
        book.merged().is_some_and(|(at, _)| at != long_ago)
    })
    .await;
    assert!(
        asked(&server, 1).starts_with("Below are your memories"),
        "只抽过一个会话也合"
    );
    chat(&home, &handle, 3, "我住在东京").await;
    chat(&home, &handle, 4, "很多年了").await;
    requested(&server, 3).await;
    // 第三个请求发出去了，抽取的回答落进日志还要一会儿。
    until(&home, |book| {
        book.all().any(|entry| entry.text == "用户住在东京")
    })
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(server.received().len(), 3, "合过的 16 小时内不再合");
    stop(&handle).await;
}

/// 抽到 0 条的不看够不够（施工 E-9：没新增就不判断，没有轮询）：间隔早过了、也有上次合并以后新记的，这一次抽取交回 0 条照样
/// 不合；下一次抽到了东西才合。
#[tokio::test]
async fn an_extraction_that_saved_nothing_does_not_start_a_merge() {
    let server = Server::start(vec![
        stream(r#"{"memories": []}"#),
        every_turn("用户住在东京"),
        merged(serde_json::json!({"summary": "用户养猫，住在东京。"})),
        spare(),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    // 很久以前合过一次，那以后又记了一条：间隔早过了，也有新记的。
    let long_ago = Timestamp::parse("2026-01-01T00:00:00.000Z").expect("合写法");
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let mark = Merged {
        upto: Seq::new(1).expect("不是零"),
        given: 0,
        revised: 0,
        retired: 0,
        failed: false,
    };
    log.append(long_ago, alice(), None, &MemoryEvent::Merged(mark))
        .expect("记得下");
    save(&home, "用户养了一只猫");
    let handle = session(&home, 4).await;
    chat(&home, &handle, 1, "今天好热").await;
    chat(&home, &handle, 2, "是啊").await;
    requested(&server, 1).await;
    until(&home, |book| book.extracted(handle.id()).is_some()).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(server.received().len(), 1, "抽到 0 条的不合");
    chat(&home, &handle, 3, "我住在东京").await;
    chat(&home, &handle, 4, "很多年了").await;
    requested(&server, 3).await;
    until(&home, |book| {
        book.merged().is_some_and(|(at, _)| at != long_ago)
    })
    .await;
    assert!(
        asked(&server, 2).starts_with("Below are your memories"),
        "抽到了东西才合"
    );
    stop(&handle).await;
}
