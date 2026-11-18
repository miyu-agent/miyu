//! 请求体是流以后的重发（施工 V-38，`src/resend.rs`）：reqwest 原来自己做的两样照它的条件、次数补回来，每次从同一份片段清单
//! 从头再造，字节一样，「发出去了」报的哈希是整份的。
//!
//! - 307、308：带着请求体跟过去；同一主机的认证头照带、带上 `Referer`；跨了主机（端口不一样）的去掉 `Authorization`；
//!   最多跟 10 次，第 11 次报 reqwest 的那句原话。303 照旧由 reqwest 自己改成不带请求体的 GET。
//! - h2 上对方没处理就拒了（`REFUSED_STREAM`、`NO_ERROR` 的 `GOAWAY`）：立刻再发，最多再发 2 次。
//! - 请求体还没送完对方就回了出错的状态：哈希照样算完整，剩下的不往外送。

use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Attempt, Body, Client, Endpoint, Outcome, Progress, Proxy, client, send};
use miyu_kernel::id::ContentHash;

use crate::support::h2c::{H2c, Step};
use crate::support::{driver, sample, with_picture};

/// 照 `client` 发 `body` 给 `base_url`，交回「发出去了」报的哈希和收场。
async fn run(client: &Client, base_url: &str, body: Body) -> (Option<ContentHash>, Outcome) {
    run_as(client, &Endpoint::new(base_url, "sk-test"), &driver(), body).await
}

/// 同 [`run`]，端点、驱动照给的。
async fn run_as(
    client: &Client,
    endpoint: &Endpoint,
    driver: &dyn miyu_drivers::Driver,
    body: Body,
) -> (Option<ContentHash>, Outcome) {
    let mut sent = None;
    let outcome = send(
        Attempt {
            client,
            endpoint,
            driver,
            body,
            path: "/chat/completions",
            idle: Duration::from_secs(20),
        },
        std::future::pending(),
        |progress| {
            if let Progress::Sent { request, .. } = progress {
                sent = Some(request);
            }
        },
    )
    .await;
    (sent, outcome)
}

fn ok(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Ended { error: None, .. })
}

fn moved(status: u16, location: &str) -> Reply {
    Reply::error(status, &[("Location", location)], "")
}

fn answer() -> Reply {
    Reply::stream(vec![Piece::Bytes(sample("openai-text"))])
}

#[tokio::test]
async fn a_307_or_308_is_followed_with_the_same_body() {
    for status in [307, 308] {
        let server =
            Server::start(vec![moved(status, "/v1/moved/chat/completions"), answer()]).await;
        let (body, whole) = with_picture();
        let client = client(Proxy::Off).expect("造得出客户端");
        let (sent, outcome) = run(&client, &server.base_url, body).await;
        assert!(ok(&outcome), "{status}: {outcome:?}");
        let received = server.received();
        assert_eq!(received.len(), 2, "{status}：跟过去了");
        assert_eq!(received[1].path, "/v1/moved/chat/completions");
        assert_eq!(received[1].method, "POST");
        assert!(
            received[0].body == whole && received[1].body == whole,
            "{status}：两回都是整份"
        );
        assert_eq!(sent, Some(ContentHash::of(&whole)));
        assert_eq!(
            received[1].header("authorization"),
            Some("Bearer sk-test"),
            "同一主机照带"
        );
        assert_eq!(
            received[1].header("referer"),
            Some(format!("{}/chat/completions", server.base_url).as_str())
        );
        assert_eq!(
            received[1].header("content-length"),
            Some(whole.len().to_string().as_str())
        );
    }
}

#[tokio::test]
async fn crossing_to_another_host_drops_the_authorization() {
    let other = Server::start(vec![answer()]).await;
    let server = Server::start(vec![moved(
        307,
        &format!("{}/chat/completions", other.base_url),
    )])
    .await;
    let (body, whole) = with_picture();
    let client = client(Proxy::Off).expect("造得出客户端");
    let (_, outcome) = run(&client, &server.base_url, body).await;
    assert!(ok(&outcome), "{outcome:?}");
    let received = other.received();
    assert!(received[0].body == whole);
    assert_eq!(
        received[0].header("authorization"),
        None,
        "端口不一样就是跨了主机"
    );
}

/// 修以前：跨主机只去掉 `Authorization` 那几样，Anthropic 的 `x-api-key`、端点另配的头照带过去（审计 #003 第 1 条）。现在驱动
/// 给的认证头、端点另配的头跨了主机都不带。
#[tokio::test]
async fn crossing_to_another_host_drops_every_credential_header() {
    let other = Server::start(vec![answer()]).await;
    let server = Server::start(vec![moved(
        307,
        &format!("{}/chat/completions", other.base_url),
    )])
    .await;
    let client = client(Proxy::Off).expect("造得出客户端");
    let endpoint = Endpoint::new(&server.base_url, "sk-ant-secret")
        .with_header("x-relay-token", "relay-secret");
    run_as(
        &client,
        &endpoint,
        &crate::support::anthropic(),
        crate::support::BODY.to_vec().into(),
    )
    .await;
    assert_eq!(
        server.received()[0].header("x-api-key"),
        Some("sk-ant-secret"),
        "头一跳照带"
    );
    let received = other.received();
    assert_eq!(received.len(), 1, "跟过去了");
    assert_eq!(received[0].header("x-api-key"), None);
    assert_eq!(received[0].header("x-relay-token"), None);
    assert_eq!(
        received[0].header("anthropic-version"),
        Some("2023-06-01"),
        "不是密钥的照带"
    );
}

/// 修以前：每一跳都从原来的头起、只和上一跳比主机，A → B → B 的第二跳又把去掉的 `Authorization` 装回去，发给了 B（审计 #003
/// 第 2 条）。现在跨过一次主机，后面每一跳都不带。
#[tokio::test]
async fn a_second_hop_on_the_other_host_does_not_get_the_key_back() {
    let other = Server::start(vec![moved(307, "/hop2/chat/completions"), answer()]).await;
    let server = Server::start(vec![moved(
        307,
        &format!("{}/hop1/chat/completions", other.base_url),
    )])
    .await;
    let client = client(Proxy::Off).expect("造得出客户端");
    let (_, outcome) = run(
        &client,
        &server.base_url,
        crate::support::BODY.to_vec().into(),
    )
    .await;
    assert!(ok(&outcome), "{outcome:?}");
    let received = other.received();
    assert_eq!(received.len(), 2, "两跳都到了");
    assert_eq!(received[1].path, "/hop2/chat/completions");
    for hop in &received {
        assert_eq!(hop.header("authorization"), None, "{}", hop.path);
    }
}

#[tokio::test]
async fn the_eleventh_redirect_is_an_error() {
    let mut replies: Vec<Reply> = (0..11)
        .map(|_| moved(307, "/v1/chat/completions"))
        .collect();
    replies.push(answer());
    let server = Server::start(replies).await;
    let client = client(Proxy::Off).expect("造得出客户端");
    let (sent, outcome) = run(
        &client,
        &server.base_url,
        crate::support::BODY.to_vec().into(),
    )
    .await;
    let Outcome::Ended {
        error: Some(failure),
        ..
    } = outcome
    else {
        panic!("该出错：{outcome:?}");
    };
    assert_eq!(
        failure.error.message,
        "error following redirect: too many redirects"
    );
    assert_eq!(server.received().len(), 11, "最初一次加十次跳转");
    assert_eq!(sent, Some(ContentHash::of(crate::support::BODY)));
}

#[tokio::test]
async fn a_303_still_becomes_a_get_without_the_body() {
    let server = Server::start(vec![moved(303, "/v1/other"), answer()]).await;
    let (body, _) = with_picture();
    let client = client(Proxy::Off).expect("造得出客户端");
    let (_, outcome) = run(&client, &server.base_url, body).await;
    assert!(ok(&outcome), "{outcome:?}");
    let received = server.received();
    assert_eq!(received[1].method, "GET");
    assert!(received[1].body.is_empty());
}

/// 事先知道是 h2 的客户端（假服务器说明文的 h2）。
fn h2_client() -> Client {
    Client::builder()
        .http2_prior_knowledge()
        .no_proxy()
        .build()
        .expect("造得出客户端")
}

#[tokio::test]
async fn a_refused_or_abandoned_stream_is_sent_again() {
    for (steps, tries) in [
        (vec![Step::Refuse], 2),
        (vec![Step::Refuse, Step::Refuse], 3),
        (vec![Step::GoAway], 2),
        (vec![Step::GoAway, Step::Refuse], 3),
    ] {
        let mut script = steps.clone();
        script.push(Step::Reply(sample("openai-text")));
        let server = H2c::start(script).await;
        let (body, whole) = with_picture();
        let (sent, outcome) = run(&h2_client(), &server.base_url, body).await;
        assert!(ok(&outcome), "{steps:?}: {outcome:?}");
        let got = server.got();
        assert_eq!(got.len(), tries, "{steps:?}");
        let last = got.last().expect("有");
        assert!(
            last.complete && last.body == whole,
            "{steps:?}：最后一回收到整份"
        );
        assert_eq!(sent, Some(ContentHash::of(&whole)), "{steps:?}");
    }
}

#[tokio::test]
async fn refused_three_times_gives_up() {
    let server = H2c::start(vec![
        Step::Refuse,
        Step::Refuse,
        Step::Refuse,
        Step::Reply(sample("openai-text")),
    ])
    .await;
    let (body, whole) = with_picture();
    let (sent, outcome) = run(&h2_client(), &server.base_url, body).await;
    assert!(!ok(&outcome), "{outcome:?}");
    assert_eq!(server.got().len(), 3, "最初一次加再发两次");
    assert_eq!(sent, Some(ContentHash::of(&whole)));
}

#[tokio::test]
async fn an_early_error_status_still_reports_the_whole_hash() {
    let server = H2c::start(vec![Step::Early(401)]).await;
    let (body, whole) = with_picture();
    let (sent, outcome) = run(&h2_client(), &server.base_url, body).await;
    let Outcome::Ended {
        error: Some(failure),
        ..
    } = outcome
    else {
        panic!("该出错：{outcome:?}");
    };
    assert_eq!(failure.error.status, Some(401));
    let got = server.got();
    assert!(
        !got[0].complete && got[0].body.len() < whole.len(),
        "对方没开窗口：最多送了 {} 个字节，整份 {}",
        got[0].body.len(),
        whole.len()
    );
    assert_eq!(sent, Some(ContentHash::of(&whole)), "剩下的生成出来算完");
}

/// 对照：reqwest 自己拿着整份的请求体（施工 V-38 以前的发法）重发的条件、次数，和上面补回来的一样。reqwest 升级改了这些，
/// 这里先红。
#[tokio::test]
async fn reqwest_itself_did_the_same_with_a_whole_body() {
    let (_, whole) = with_picture();
    for (steps, tries, answered) in [
        (vec![Step::Refuse, Step::Refuse], 3, true),
        (vec![Step::GoAway, Step::Refuse], 3, true),
        (vec![Step::Refuse, Step::Refuse, Step::Refuse], 3, false),
    ] {
        let mut script = steps.clone();
        script.push(Step::Reply(sample("openai-text")));
        let server = H2c::start(script).await;
        let sent = h2_client()
            .post(format!("{}/chat/completions", server.base_url))
            .body(whole.clone())
            .send()
            .await;
        assert_eq!(sent.is_ok(), answered, "{steps:?}");
        assert_eq!(server.got().len(), tries, "{steps:?}");
    }
    let mut replies: Vec<Reply> = (0..11)
        .map(|_| moved(308, "/v1/chat/completions"))
        .collect();
    replies.push(answer());
    let server = Server::start(replies).await;
    let client = client(Proxy::Off).expect("造得出客户端");
    let error = client
        .post(format!("{}/chat/completions", server.base_url))
        .body(whole.clone())
        .send()
        .await
        .expect_err("跳了十一次");
    assert!(error.is_redirect());
    assert_eq!(server.received().len(), 11);
    assert!(
        server
            .received()
            .iter()
            .all(|request| request.body == whole)
    );
}
