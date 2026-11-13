//! 人生结束了不再抽取（施工 LF-4 上，`docs/blueprint/memory.md` 第六条、`life.md`「结束」）：结束以后封存当场响的闹钟不抽；
//! 抽取正在半路上时结束的，那一次交回来的整批丢掉。不然按会话清只清「那以前」的，这段人生会被抽回记忆。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::event::{Body, LifeEnded};
use miyu_kernel::session::{Appended, Command};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 人记下 `life.ended`，等它落盘、会话封存。
async fn end_life(home: &Home, handle: &Handle) {
    let ended = Command::Append {
        event: Appended::LifeEnded(LifeEnded { last_words: None }),
    };
    within("回应", handle.command(id("end"), alice(), ended))
        .await
        .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .any(|event| matches!(event.body, Body::SessionSealed(_)))
    })
    .await;
}

#[tokio::test]
async fn an_ended_life_is_not_extracted_when_the_sealed_alarm_rings() {
    // 闲一小时才抽：封存了的闹钟当场响（施工 V-23），没有这一条拦着的话马上就抽。
    let server = Server::start(vec![every_turn("她今天去了海边")]).await;
    let mut home = Home::new();
    organizer_after(&mut home, &server, "", Duration::from_secs(3600));
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "早上好").await;
    chat(&home, &handle, 2, "去海边吧").await;
    end_life(&home, &handle).await;
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(server.received().is_empty(), "结束了的不发抽取的请求");
    assert!(extracted(&home).is_empty(), "记忆里没有这段人生");
    stop(&handle).await;
}

#[tokio::test]
async fn an_extraction_on_its_way_when_the_life_ends_is_dropped() {
    // 交回的候选照每一轮各一条（同 `every_turn`），晚 800 毫秒才回：回来时人生已经结束了。
    let memories: Vec<serde_json::Value> = (1..=TURNS)
        .map(|turn| serde_json::json!({"class": "user", "text": "她今天去了海边", "turn": turn}))
        .collect();
    let late = serde_json::json!({ "memories": memories }).to_string();
    let server = Server::start(vec![stream_after(&late, Duration::from_millis(800))]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "早上好").await;
    chat(&home, &handle, 2, "去海边吧").await;
    requested(&server, 1).await;
    end_life(&home, &handle).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(extracted(&home).is_empty(), "交回来的整批丢掉");
    assert_eq!(mark(&home, handle.id()), None, "也不记抽到了哪");
    stop(&handle).await;
}
