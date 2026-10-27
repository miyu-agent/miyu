//! 一直不闲的会话也抽（施工 E-5，`docs/blueprint/memory.md` 第六条第 1 款）：闲多久设成一小时（等于一直不闲），上次抽以后结束了
//! `memory.extract_max_turns` 轮、或者最早那一轮结束等满 `memory.extract_max_wait`，不等闲就抽；正在跑的那一轮不交、留给下次。
//! 2026-10-27 主人本机 QQ 主群一下午 299 轮、最长只停 10.7 分钟，闲 30 分钟才抽的一次都没抽。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::session::Command;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

/// 抽取的请求（合并、写关键词的不算）。
fn extractions(server: &Server) -> Vec<String> {
    (0..server.received().len())
        .map(|at| asked(server, at))
        .filter(|text| text.starts_with("Find what is worth remembering"))
        .collect()
}

/// 等到抽取的请求有 `count` 个（最多 10 秒）。
async fn extracted_times(server: &Server, count: usize) -> Vec<String> {
    for _ in 0..200 {
        let found = extractions(server);
        if found.len() >= count {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let all: Vec<(String, String)> = server
        .received()
        .iter()
        .map(|one| {
            (
                one.path.clone(),
                String::from_utf8_lossy(&one.body)
                    .chars()
                    .take(120)
                    .collect(),
            )
        })
        .collect();
    panic!(
        "十秒没等到第 {count} 次抽取，有 {}：{all:#?}",
        extractions(server).len()
    );
}

fn turn_tag(turn: miyu_kernel::id::TurnId) -> String {
    format!("<turn number=\"{}\"", turn.started().get())
}

#[tokio::test]
async fn a_busy_session_is_extracted_every_few_turns() {
    let server = Server::start(vec![every_turn("用户在聊天"), every_turn("用户还在聊天")]).await;
    let mut home = Home::new();
    organizer_busy(&mut home, &server, "extract_max_turns = 3\n", None);
    let script = Script::new((0..6).map(|_| Play::Says("好。")));
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "第一句").await;
    chat(&home, &handle, 2, "第二句").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(extractions(&server).is_empty(), "两轮不到三轮");
    chat(&home, &handle, 3, "第三句").await;
    let first = extracted_times(&server, 1).await;
    let turns = ended_turns(&home, handle.id());
    for turn in &turns[..3] {
        assert!(first[0].contains(&turn_tag(*turn)), "{}", first[0]);
    }
    chat(&home, &handle, 4, "第四句").await;
    chat(&home, &handle, 5, "第五句").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(extractions(&server).len(), 1, "上次以后两轮，不到三轮");
    chat(&home, &handle, 6, "第六句").await;
    let second = extracted_times(&server, 2).await;
    let turns = ended_turns(&home, handle.id());
    for turn in &turns[3..6] {
        assert!(second[1].contains(&turn_tag(*turn)), "{}", second[1]);
    }
    assert!(!second[1].contains("第三句"), "抽过的不重抽：{}", second[1]);
    stop(&handle).await;
}

#[tokio::test]
async fn a_busy_session_is_extracted_once_the_earliest_turn_has_waited_enough() {
    let server = Server::start(vec![every_turn("用户在聊天")]).await;
    let mut home = Home::new();
    organizer_busy(
        &mut home,
        &server,
        "extract_max_turns = 500\n",
        Some(Duration::from_millis(400)),
    );
    // 第三轮一直停着：不等闲的那一次取到读的那一刻结束了的轮，第三轮要是很快答完（CI 慢的时候读在它结束以后），带上它也对，
    // 只是这条测试就看不出「第三轮留给下次」了（施工 E-5 修：Windows、macOS 上偶发红过）。
    let script = Script::new([Play::Says("好。"), Play::Says("好。"), Play::Holds]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "第一句").await;
    chat(&home, &handle, 2, "第二句").await;
    assert!(extractions(&server).is_empty(), "还没等够");
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(extractions(&server).is_empty(), "没有新的一批不看");
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: "第三句".to_string(),
        })],
        urgent: false,
        venue: None,
    };
    handle
        .command(id("cmd-3"), alice(), said)
        .await
        .expect("会话在跑");
    let found = extracted_times(&server, 1).await;
    // 第三轮开始的那一批看出等够了：抽到那时结束了的前两轮，第三轮还在跑、留给下次。
    assert!(
        found[0].contains("第一句") && found[0].contains("第二句"),
        "{}",
        found[0]
    );
    assert!(!found[0].contains("第三句"), "{}", found[0]);
    stop(&handle).await;
}

#[tokio::test]
async fn the_running_turn_is_left_for_next_time_and_a_new_turn_does_not_void_it() {
    // 第二轮结束就该抽了，紧接着第三轮开始、一直停在那儿：会话一直不闲，照样抽前两轮，第三轮的话不交。
    let server = Server::start(vec![every_turn("用户在聊天")]).await;
    let mut home = Home::new();
    organizer_busy(&mut home, &server, "extract_max_turns = 2\n", None);
    let script = Script::new([Play::Says("好。"), Play::Says("好。"), Play::Holds]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "第一句").await;
    chat(&home, &handle, 2, "第二句").await;
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: "还在说的第三句".to_string(),
        })],
        urgent: false,
        venue: None,
    };
    handle
        .command(id("cmd-3"), alice(), said)
        .await
        .expect("会话在跑");
    let found = extracted_times(&server, 1).await;
    assert!(
        found[0].contains("第一句") && found[0].contains("第二句"),
        "{}",
        found[0]
    );
    assert!(
        !found[0].contains("还在说的第三句"),
        "正在跑的那一轮不交：{}",
        found[0]
    );
    for _ in 0..100 {
        if mark(&home, handle.id()).is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let turns = ended_turns(&home, handle.id());
    assert_eq!(turns.len(), 2, "第三轮还在跑");
    let upto = mark(&home, handle.id()).expect("记下了抽到哪");
    let log = home.log(handle.id());
    let third = log
        .iter()
        .find(|event| {
            matches!(&event.body, miyu_kernel::event::Body::MessageUser(user)
                if user.blocks.iter().any(|block| matches!(block, Block::Text(text) if text.text == "还在说的第三句")))
        })
        .expect("第三句落了盘");
    assert!(upto < third.seq, "抽到哪停在第三句以前，下次接着");
    stop(&handle).await;
}

#[tokio::test]
async fn a_session_that_never_goes_idle_is_still_extracted() {
    // 第一轮结束以后等满，第二轮一开始就一直停着：看出该抽的那一批、读回来的那一刻会话都忙着，照样抽第一轮。
    let server = Server::start(vec![every_turn("用户在聊天")]).await;
    let mut home = Home::new();
    organizer_busy(
        &mut home,
        &server,
        "extract_turns = 1\nextract_max_turns = 500\n",
        Some(Duration::from_millis(300)),
    );
    let script = Script::new([Play::Says("好。"), Play::Holds]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "第一句").await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: "一直在说的第二句".to_string(),
        })],
        urgent: false,
        venue: None,
    };
    handle
        .command(id("cmd-2"), alice(), said)
        .await
        .expect("会话在跑");
    let found = extracted_times(&server, 1).await;
    assert!(found[0].contains("第一句"), "{}", found[0]);
    assert!(!found[0].contains("一直在说的第二句"), "{}", found[0]);
    assert_eq!(ended_turns(&home, handle.id()).len(), 1, "第二轮一直在跑");
    stop(&handle).await;
}
