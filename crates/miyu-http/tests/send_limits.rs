//! 读流的两道时限（施工 AE-2，发布前审计 10-30）：空闲超时量的是多久没解出新的内容，一直发保活注释的照样超时；内容一直
//! 在来、整次请求超过空闲超时的 [`TOTAL_FACTOR`] 倍的，照总超时收尾。两条修之前都一直读下去。

use std::time::Duration;

use crate::support::{BODY, driver, sample};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Attempt, Endpoint, Outcome, Proxy, TOTAL_FACTOR, client, send};
use miyu_kernel::event::ErrorClass;

/// 空闲时限。
const IDLE: Duration = Duration::from_millis(300);
/// 假服务器多久发一片：在空闲时限以内。
const BEAT: Duration = Duration::from_millis(50);
/// 最多等多久：总时限（3 秒）再宽一些。
const PATIENCE: Duration = Duration::from_secs(10);

/// 第 `n` 条事件（从 0 数）在样本里的起止。
fn event(stream: &[u8], n: usize) -> std::ops::Range<usize> {
    let mut start = 0;
    for index in 0..=n {
        let end = start
            + stream[start..]
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .unwrap_or_else(|| panic!("有第 {index} 条"))
            + 4;
        if index == n {
            return start..end;
        }
        start = end;
    }
    unreachable!()
}

/// 照剧本 `body` 回，等它收场，交回出错的原话；`PATIENCE` 内没收场的、没出错的照失败。
async fn failure(body: Vec<Piece>) -> String {
    let server = Server::start(vec![Reply::stream(body)]).await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let client = client(Proxy::Off).expect("造得出客户端");
    let driver = driver();
    let sent = send(
        Attempt {
            client: &client,
            endpoint: &endpoint,
            driver: &driver,
            body: BODY,
            path: "/chat/completions",
            idle: IDLE,
        },
        std::future::pending(),
        |_| {},
    );
    let outcome = tokio::time::timeout(PATIENCE, sent)
        .await
        .unwrap_or_else(|_| panic!("{PATIENCE:?} 内没有收场"));
    let Outcome::Ended {
        error: Some(error), ..
    } = outcome
    else {
        panic!("应该出错：{outcome:?}");
    };
    assert_eq!(error.error.class, ErrorClass::Retryable);
    error.error.message
}

#[tokio::test]
async fn keep_alives_without_content_still_time_out() {
    let stream = sample("openai-text");
    // 先照常来第一条事件（响应开了头），之后只剩保活，一直发到等不及。
    let mut body = vec![Piece::Bytes(stream[event(&stream, 0)].to_vec())];
    for _ in 0..400 {
        body.push(Piece::Wait(BEAT));
        body.push(Piece::Bytes(b": keep-alive\r\n\r\n".to_vec()));
    }
    let message = failure(body).await;
    assert!(message.contains("空闲超时"), "{message}");
}

#[tokio::test]
async fn a_reply_that_never_ends_stops_at_the_total() {
    let stream = sample("openai-text");
    // 带内容的那一条（第二条，「你好」）一直重复：每片都有新内容，空闲超时不到，只有总时限停得下。
    let said = stream[event(&stream, 1)].to_vec();
    let mut body = vec![Piece::Bytes(stream[event(&stream, 0)].to_vec())];
    for _ in 0..400 {
        body.push(Piece::Wait(BEAT));
        body.push(Piece::Bytes(said.clone()));
    }
    let started = std::time::Instant::now();
    let message = failure(body).await;
    assert!(message.contains("总超时"), "{message}");
    assert!(
        started.elapsed() >= IDLE * TOTAL_FACTOR,
        "到了总时限才停：{:?}",
        started.elapsed()
    );
}
