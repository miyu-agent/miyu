use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::{Expired, Log, events, open};
use crate::testkit::http::HttpServer;
use crate::{Client, Era, Failed, Hello, Waits};

fn hello() -> Hello {
    Hello {
        name: "miyu".to_string(),
        version: "0".to_string(),
    }
}

/// 一份记日志的：交回它和记下的。
fn log() -> (Log, Arc<Mutex<Vec<String>>>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&lines);
    (Arc::new(move |line| kept.lock().unwrap().push(line)), lines)
}

/// 记下的里有没有以 `prefix` 开头的一行。
fn logged(lines: &Mutex<Vec<String>>, prefix: &str) -> bool {
    lines
        .lock()
        .unwrap()
        .iter()
        .any(|line| line.starts_with(prefix))
}

/// 带着令牌和一个请求头连 `server`：交回客户端（或者连不上），和这条连接是不是过期断的。
async fn connect(server: &HttpServer, log: Log) -> (Result<Client, Failed>, Expired) {
    let headers = vec![
        ("Authorization".to_string(), "Bearer t0k".to_string()),
        ("X-Team".to_string(), "core".to_string()),
    ];
    let opened = open(
        miyu_http::client(miyu_http::Proxy::Off).expect("客户端"),
        server.url.clone(),
        headers,
        log,
    );
    let expired = opened.expired.clone();
    let client = Client::connect(opened.read, opened.write, hello(), None, Waits::default()).await;
    (client, expired)
}

#[test]
fn server_sent_events_split_into_messages() {
    let body = "event: message\ndata: {\"a\":1}\n\n: comment\ndata: {\"b\":\r\ndata: 2}\r\n\r\ndata: {\"c\":3}";
    assert_eq!(events(body), ["{\"a\":1}", "{\"b\":\n2}", "{\"c\":3}"]);
}

#[tokio::test]
async fn a_url_server_is_discovered_listed_and_called() {
    for sse in [false, true] {
        let server = HttpServer::start(sse).await;
        let (log, lines) = log();
        let client = connect(&server, log).await.0.expect("连得上");
        assert_eq!(
            client.era(),
            &Era::Legacy("2025-06-18".to_string()),
            "回不认识 discover 的退回握手"
        );
        let listed = client.tools().await.expect("列得出");
        assert_eq!(listed.tools[0].name, "echo");
        let called = client
            .call("echo", json!({"text": "hi"}))
            .await
            .expect("调得到");
        assert_eq!(called.content.len(), 1, "{called:?}");
        let received = server.received();
        assert!(
            received
                .iter()
                .all(|request| request.header("authorization") == Some("Bearer t0k")),
            "令牌每次都带"
        );
        assert!(
            received
                .iter()
                .all(|request| request.header("x-team") == Some("core"))
        );
        let shake = received
            .iter()
            .find(|request| request.body["method"] == "initialize")
            .expect("握了手");
        assert_eq!(
            shake.header("mcp-protocol-version"),
            None,
            "老时代的握手那一次不带版本头（施工 X-32 补）"
        );
        let after = received
            .iter()
            .skip_while(|request| request.body["method"] != "initialize")
            .skip(1);
        for request in after {
            assert_eq!(
                request.header("mcp-session-id"),
                Some("s-1"),
                "握手以后带会话编号：{request:?}"
            );
            assert_eq!(
                request.header("mcp-protocol-version"),
                Some("2025-06-18"),
                "带认出来的版本"
            );
        }
        assert!(
            lines.lock().unwrap().is_empty(),
            "discover 回的是 JSON-RPC 的错，不是 HTTP 的错"
        );
    }
}

#[tokio::test]
async fn a_refusing_server_fails_the_connect_and_says_why() {
    let server = HttpServer::start(false).await;
    server.refuse(401);
    let (log, lines) = log();
    assert!(connect(&server, log).await.0.is_err(), "401 连不上");
    assert!(logged(&lines, "HTTP 401"), "{:?}", lines.lock().unwrap());
}

#[tokio::test]
async fn a_server_error_answers_the_request_at_once() {
    let server = HttpServer::start(false).await;
    let (log, lines) = log();
    let client = connect(&server, log).await.0.expect("连得上");
    server.refuse(500);
    let called = tokio::time::timeout(
        Duration::from_secs(5),
        client.call("echo", json!({"text": "hi"})),
    )
    .await
    .expect("马上回，不干等到超时");
    match called {
        Err(Failed::Rpc { message, .. }) => assert!(message.starts_with("HTTP 500"), "{message}"),
        other => panic!("该是带原话的错：{other:?}"),
    }
    assert!(logged(&lines, "HTTP 500"));
}

#[tokio::test]
async fn an_expired_session_closes_the_connection() {
    let server = HttpServer::start(false).await;
    let (log, lines) = log();
    let (client, expired) = connect(&server, log).await;
    let client = client.expect("连得上");
    assert!(!expired.happened());
    server.expire();
    assert!(
        client.call("echo", json!({"text": "hi"})).await.is_err(),
        "过期的这一次失败"
    );
    tokio::time::timeout(Duration::from_secs(5), client.closed())
        .await
        .expect("连接断了");
    assert!(expired.happened(), "看管的一头认得出是过期断的");
    assert!(logged(&lines, "session expired"));
}

#[tokio::test]
async fn an_oversized_answer_fails_the_request_instead_of_filling_memory() {
    let server = HttpServer::start(false).await;
    let (log, lines) = log();
    let client = connect(&server, log).await.0.expect("连得上");
    server.oversize();
    let called = tokio::time::timeout(
        Duration::from_secs(30),
        client.call("echo", json!({"text": "hi"})),
    )
    .await
    .expect("马上回，不干等");
    match called {
        Err(Failed::Rpc { message, .. }) => {
            assert!(message.starts_with("the answer is longer"), "{message}");
        }
        other => panic!("该是太长的错：{other:?}"),
    }
    assert!(logged(&lines, "the answer is longer"));
    assert!(
        client.call("echo", json!({"text": "hi"})).await.is_ok(),
        "连接照旧能用"
    );
}

/// 新时代经网址（施工 X-32 补，GitHub 的远程服务撞出来的）：每个请求照它 `_meta` 里的版本带 `MCP-Protocol-Version`，认成新时代、
/// 列、调都通；服务的日志里没有 `400`。
#[tokio::test]
async fn a_modern_url_server_gets_the_version_header_on_every_request() {
    for sse in [false, true] {
        let server = HttpServer::modern(sse).await;
        let (log, lines) = log();
        let client = connect(&server, log).await.0.expect("连得上");
        assert_eq!(
            client.era(),
            &Era::Modern(crate::MODERN[0].to_string()),
            "认成新时代：{:?}",
            lines.lock().unwrap()
        );
        let listed = client.tools().await.expect("列得出");
        assert_eq!(listed.tools[0].name, "echo");
        let called = client
            .call("echo", json!({"text": "hi"}))
            .await
            .expect("调得到");
        assert_eq!(called.content.len(), 1, "{called:?}");
        for request in server.received() {
            let meta =
                request.body["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str();
            assert_eq!(request.header("mcp-protocol-version"), meta, "{request:?}");
        }
        assert!(
            !logged(&lines, "HTTP 400"),
            "没有被拒的：{:?}",
            lines.lock().unwrap()
        );
    }
}
