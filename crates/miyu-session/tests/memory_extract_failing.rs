//! 抽取连着出错（施工 E-5 补，`docs/blueprint/memory.md` 第六条第 1 款）：不等闲的那一路每失败一次要攒的轮数翻一倍，坏着的
//! 模型不再每攒满一批就整份读一遍日志、发一次请求；成了一次复位；闲了的那一路照旧，闲够了就试。

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::{Piece, Server};
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
///
/// 不等闲的那一次从开始抽数轮，在路上的没回来不开新的：慢的机器上第几轮抽不固定（Windows 上第一次出错晚于第 6 轮才回来，
/// 第三次就到第 15 轮以后）。第一次出错的回答压到第 7 轮以后才放，每次都走慢的那条路；第几轮抽不写死，照成了的那一次读到哪一轮
/// 往后数。
#[tokio::test]
async fn one_success_resets_the_wait() {
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let mut replies = garbled(2);
    replies[0].body.insert(0, Piece::Gate(Arc::clone(&gate)));
    replies.extend([every_turn("用户在聊天"), every_turn("用户还在聊天")]);
    let server = Server::start(replies).await;
    let mut home = Home::new();
    organizer_busy(&mut home, &server, "extract_max_turns = 2\n", None);
    let script = Script::new((0..LAST + 6).map(|_| Play::Says("好。")));
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    for n in 1..=7 {
        chat(&home, &handle, n, &format!("第{n}句")).await;
    }
    extracted_times(&server, 1).await;
    // 第 2 轮那一次压到这时才出错：在路上的时候不开新的。
    if tokio::time::timeout(Duration::from_secs(20), gate.wait())
        .await
        .is_err()
    {
        panic!("二十秒没放出第一次抽取的回答");
    }
    // 出错两次（要攒的轮数 2、4、8），第三次成了：第几轮看在路上的几次什么时候回来。
    let mut n = 7;
    while !extracted_within(&server, 3, Duration::from_millis(500)).await {
        n += 1;
        assert!(
            n <= LAST,
            "到第 {LAST} 轮还没抽第 3 次，有 {}",
            extractions(&server)
        );
        chat(&home, &handle, n, &format!("第{n}句")).await;
    }
    // 成了的那一次读到第 `read` 轮：开始抽在那以后，复位了的再结束两轮就抽（成了的那一次补完关键词、向量才算走完，慢的机器上
    // 要好一会儿：照轮数判，不照时间）；没复位的要攒满八轮，最早到第 `read + 8` 轮。
    let read = last_said(&asked_nth(&server, 3));
    for n in n + 1..=read + 6 {
        chat(&home, &handle, n, &format!("第{n}句")).await;
        if extracted_within(&server, 4, Duration::from_secs(2)).await {
            stop(&handle).await;
            return;
        }
    }
    panic!(
        "成了的那一次读到第 {read} 轮，到第 {} 轮还没抽第 4 次：失败的次数没复位",
        read + 6
    );
}

/// [`one_success_resets_the_wait`] 最多聊到第几轮。
const LAST: usize = 40;

/// 第 `count` 个抽取的请求的那一段。
fn asked_nth(server: &Server, count: usize) -> String {
    (0..server.received().len())
        .map(|at| asked(server, at))
        .filter(|text| text.starts_with("Find what is worth remembering"))
        .nth(count - 1)
        .unwrap_or_else(|| panic!("没有第 {count} 个抽取的请求"))
}

/// 这一段里最后是人说的第几句（`第{n}句`）。
fn last_said(text: &str) -> usize {
    (1..=LAST)
        .rev()
        .find(|n| text.contains(&format!("第{n}句")))
        .unwrap_or_else(|| panic!("这一段里没有人说的：{text}"))
}

/// 等抽取的请求到 `count` 个，最多等 `wait`：交回等到没有。
async fn extracted_within(server: &Server, count: usize, wait: Duration) -> bool {
    let until = tokio::time::Instant::now() + wait;
    while tokio::time::Instant::now() < until {
        if extractions(server) >= count {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    extractions(server) >= count
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
