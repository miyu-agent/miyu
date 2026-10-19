//! 叫停人叫的整理（施工 R-22，`docs/blueprint/memory.md` 第七条第 13 款）：真会话、真记忆日志，整理记忆的模型是一台假服务器
//! （`org/m`）。会话的 `/stop`（`Handle::stop_jobs`）连这个会话正在跑的整理一起停：合完了的批留着，正在合的那一批放弃、记一条
//! `stopped`（不数次数），交回 `stopped`；下次整理从放弃的那一批接着合。不经会话的那一趟听不见。

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::id::Seq;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{MemoryEvent, MergeFailed, STOPPED, from_event};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Dreamed, Handle, Keeper, Listener, Turn};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona, save};
use crate::support::*;

/// 合并的一段：一直不回，等对方断开。
fn stalled() -> Reply {
    let mut reply = stream("{}");
    reply.body.insert(0, Piece::Stall);
    reply
}

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

/// 这一间记过的整理事件：开始合到哪、合完到哪、没合成的。
fn merges(home: &Home) -> Vec<MemoryEvent> {
    keeper(home)
        .follow(Some(0), 4096)
        .expect("跟得上")
        .filled
        .iter()
        .filter_map(|event| from_event(event)?.ok())
        .filter(|event| {
            matches!(
                event,
                MemoryEvent::Merging(_) | MemoryEvent::Merged(_) | MemoryEvent::MergeFailed(_)
            )
        })
        .collect()
}

async fn session(home: &Home) -> Handle {
    home.create_full(
        &Script::new([Play::Says("好。")]),
        &catalog(home),
        Opening::default(),
        Lines::default(),
    )
    .await
}

/// 不经会话的那一趟：照场地这时的配置。
async fn dream(home: &Home) -> Dreamed {
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(home)
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await
        .expect("整理了")
}

#[tokio::test]
async fn a_stop_keeps_what_was_merged_and_drops_the_batch_being_merged() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        stream(&serde_json::json!({"revised": [{"id": "m1", "text": "改过的第一条"}]}).to_string()),
        stalled(),
        stream("{}"),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 5\n");
    // 一百条长的（一条三百多字节）：一次放不下 32 KiB，要合两批。
    for n in 0..100 {
        save(&home, &format!("第 {n} 条：{}", "很长的一句话".repeat(19)));
    }
    let handle = session(&home).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    let dreaming = tokio::spawn({
        let handle = handle.clone();
        async move { handle.dream().await }
    });
    within("第二批发出去了", server.wait_stalled(1)).await;
    handle.stop_jobs(alice(), id("s1")).await.expect("会话在跑");
    let dreamed = within("叫停了就回", dreaming)
        .await
        .expect("没崩")
        .expect("会话在跑")
        .expect("合完的照样交回");
    assert!(dreamed.stopped, "{dreamed:?}");
    assert_eq!(dreamed.revised, 1, "头一批合完的留着：{dreamed:?}");
    assert!(dreamed.given > 0 && dreamed.given < 101, "{dreamed:?}");
    let logged = merges(&home);
    let [
        ..,
        MemoryEvent::Merging(merging),
        MemoryEvent::MergeFailed(failed),
    ] = &logged[..]
    else {
        panic!("正在合的那一批记 stopped：{logged:?}");
    };
    assert_eq!(
        failed,
        &MergeFailed {
            upto: merging.upto,
            why: STOPPED.to_string(),
            tries: None,
        },
        "不数次数"
    );
    let first = logged
        .iter()
        .find_map(|event| match event {
            MemoryEvent::Merged(merged) => Some(merged.upto),
            _ => None,
        })
        .expect("头一批合完了");
    assert!(first < merging.upto, "放弃的是后一批");
    // 下次整理从放弃的那一批接着合。
    let next = dream(&home).await;
    assert!(!next.stopped && next.given > 0, "{next:?}");
    assert!(asked(&server, 3).contains("第 99 条"), "接着合放弃的那一批");
    let logged = merges(&home);
    assert!(
        matches!(logged.last(), Some(MemoryEvent::Merged(merged)) if merged.upto > Seq::new(90).expect("从 1 起")),
        "这回合到底了：{logged:?}"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn a_stop_while_extracting_merges_nothing() {
    let mut extracting = every_turn("用户养了一只猫");
    extracting
        .body
        .insert(0, Piece::Wait(Duration::from_millis(500)));
    let server = Server::start(vec![extracting, stream("{}")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 5\n");
    let handle = session(&home).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    let dreaming = tokio::spawn({
        let handle = handle.clone();
        async move { handle.dream().await }
    });
    requested(&server, 1).await;
    handle.stop_jobs(alice(), id("s1")).await.expect("会话在跑");
    let dreamed = within("抽完就回", dreaming)
        .await
        .expect("没崩")
        .expect("会话在跑")
        .expect("交回了");
    assert_eq!(
        dreamed,
        Dreamed {
            stopped: true,
            ..Dreamed::default()
        },
        "抽的照样抽完，不合"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(server.received().len(), 1, "没发合并的请求");
    assert!(merges(&home).is_empty(), "什么都没开始合");
    stop(&handle).await;
}
