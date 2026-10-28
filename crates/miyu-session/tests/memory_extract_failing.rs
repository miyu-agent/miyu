//! 抽取连着出错（施工 E-5 补，`docs/blueprint/memory.md` 第六条第 1 款）：不等闲的那一路每失败一次要攒的轮数翻一倍，坏着的
//! 模型不再每攒满一批就整份读一遍日志、发一次请求；成了一次复位；闲了的那一路照旧，闲够了就试。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

/// 抽取的请求有几个（合并、写关键词的不算）。
fn extractions(server: &Server) -> usize {
    (0..server.received().len())
        .map(|at| asked(server, at))
        .filter(|text| text.starts_with("Find what is worth remembering"))
        .count()
}

/// 等到抽取的请求有 `count` 个（最多 10 秒）。
async fn extracted_times(server: &Server, count: usize) {
    for _ in 0..200 {
        if extractions(server) >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("十秒没等到第 {count} 次抽取，有 {}", extractions(server));
}

/// 交回的不是 JSON：抽取当失败。
fn garbled(count: usize) -> Vec<miyu_http::testkit::Reply> {
    (0..count).map(|_| stream("没有 JSON")).collect()
}

/// 一直出错：攒两轮抽一次的会话 60 轮里只试四次（第 2、6、14、30 轮），不翻倍的要试三十次。
#[tokio::test]
async fn a_failing_model_is_tried_less_and_less_often() {
    let server = Server::start(garbled(40)).await;
    let mut home = Home::new();
    organizer_busy(&mut home, &server, "extract_max_turns = 2\n", None);
    let script = Script::new((0..60).map(|_| Play::Says("好。")));
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    for n in 1..=60 {
        chat(&home, &handle, n, &format!("第{n}句")).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let tried = extractions(&server);
    assert!((3..=4).contains(&tried), "越等越久：试了 {tried} 次");
    stop(&handle).await;
}

/// 出错两次以后成了一次：要攒的轮数复位，又是攒两轮就抽。
#[tokio::test]
async fn one_success_resets_the_wait() {
    let mut replies = garbled(2);
    replies.extend([every_turn("用户在聊天"), every_turn("用户还在聊天")]);
    let server = Server::start(replies).await;
    let mut home = Home::new();
    organizer_busy(&mut home, &server, "extract_max_turns = 2\n", None);
    let script = Script::new((0..16).map(|_| Play::Says("好。")));
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    for n in 1..=14 {
        chat(&home, &handle, n, &format!("第{n}句")).await;
    }
    // 第 2、6 轮各出错一次，第 14 轮那一次成了。
    extracted_times(&server, 3).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    chat(&home, &handle, 15, "第15句").await;
    chat(&home, &handle, 16, "第16句").await;
    extracted_times(&server, 4).await;
    stop(&handle).await;
}

/// 闲了的那一路照旧：出错以后闲够了就再试，不跟着翻倍。
#[tokio::test]
async fn an_idle_session_retries_as_before() {
    let server = Server::start(garbled(5)).await;
    let mut home = Home::new();
    organizer_after(
        &mut home,
        &server,
        "extract_turns = 1\n",
        Duration::from_millis(100),
    );
    let handle = home
        .create_full(
            &Script::new((0..3).map(|_| Play::Says("好。"))),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    for n in 1..=3 {
        chat(&home, &handle, n, &format!("第{n}句")).await;
        extracted_times(&server, n).await;
    }
    stop(&handle).await;
}
