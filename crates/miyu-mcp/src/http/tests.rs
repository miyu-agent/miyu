use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::{Expired, Log, events, header_value, open, rpc_error, standard_headers};
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
        None,
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
            // 规范的标准请求头（施工 PT-1 上）：假服务照规范查，缺了、对不上的回 `400`。
            assert_eq!(
                request.header("mcp-method"),
                request.body["method"].as_str(),
                "{request:?}"
            );
        }
        let call = server
            .received()
            .into_iter()
            .find(|request| request.body["method"] == "tools/call")
            .expect("调过");
        assert_eq!(call.header("mcp-name"), Some("echo"));
        assert!(
            !logged(&lines, "HTTP 400"),
            "没有被拒的：{:?}",
            lines.lock().unwrap()
        );
    }
}

/// 要登录的服务（施工 X-42 上）：没带对令牌的回 `401`，`WWW-Authenticate` 的原文记下来，看管的一头照它判。
#[tokio::test]
async fn a_401_keeps_its_challenge() {
    let server = HttpServer::start(false).await;
    server.protect("http://127.0.0.1:1", "right");
    let (log, _lines) = log();
    let opened = open(
        miyu_http::client(miyu_http::Proxy::Off).expect("客户端"),
        server.url.clone(),
        vec![("Authorization".to_string(), "Bearer wrong".to_string())],
        log,
        None,
    );
    let challenged = opened.challenged.clone();
    assert_eq!(challenged.header(), None, "没回过 401 的没有");
    let connected =
        Client::connect(opened.read, opened.write, hello(), None, Waits::default()).await;
    assert!(connected.is_err(), "令牌不对连不上");
    let header = challenged.header().expect("记下了挑战");
    assert!(
        header.starts_with("Bearer resource_metadata=")
            && header.contains("/.well-known/oauth-protected-resource/mcp"),
        "{header}"
    );
}

mod credentials;

/// 只认别的版本的新版服务（施工 PT-1 上）：`400` 回来的 `-32022` 原样交回，探时代的认出是新版、没有共同的版本就说清楚，不退回老
/// 握手（规范：认得出的新版错误不许退回 `initialize`）。
#[tokio::test]
async fn a_modern_server_with_other_versions_is_not_mistaken_for_an_old_one() {
    let server = HttpServer::modern(false).await;
    server.support(&["2099-01-01"]);
    let (log, lines) = log();
    let failed = connect(&server, log).await.0.err().expect("连不上");
    assert!(
        matches!(&failed, Failed::NoCommonVersion(offered) if offered == &["2099-01-01".to_string()]),
        "{failed:?}"
    );
    assert!(
        server
            .received()
            .iter()
            .all(|request| request.body["method"] != "initialize"),
        "不退回握手"
    );
    assert!(
        logged(&lines, "HTTP 400: -32022"),
        "{:?}",
        lines.lock().unwrap()
    );
}

/// 老服务对 `server/discover` 回 `400`、回应体是一条没有编号的老错误（官方 TypeScript SDK 的老服务没会话就这样，施工 PT-1 上）：
/// 原样交回、编号换成这次的，认不出是新版的错，退回握手照常用。
#[tokio::test]
async fn an_old_server_refusing_discover_with_400_falls_back_to_the_handshake() {
    let server = HttpServer::start(true).await;
    server.discover_400();
    let (log, lines) = log();
    let client = connect(&server, log).await.0.expect("连得上");
    assert_eq!(client.era(), &Era::Legacy("2025-06-18".to_string()));
    assert!(
        logged(&lines, "HTTP 400: -32000 Bad Request"),
        "{:?}",
        lines.lock().unwrap()
    );
    let called = client
        .call("echo", json!({"text": "hi"}))
        .await
        .expect("调得到");
    assert_eq!(called.content.len(), 1);
}

#[test]
fn header_values_follow_the_spec_encoding() {
    for (plain, sent) in [
        ("us-west1", "us-west1"),
        ("Hello, 世界", "=?base64?SGVsbG8sIOS4lueVjA==?="),
        (" padded ", "=?base64?IHBhZGRlZCA=?="),
        ("line1\nline2", "=?base64?bGluZTEKbGluZTI=?="),
        ("=?base64?literal?=", "=?base64?PT9iYXNlNjQ/bGl0ZXJhbD89?="),
    ] {
        assert_eq!(header_value(plain), sent, "{plain:?}");
    }
}

#[test]
fn standard_headers_name_the_method_and_what_it_acts_on() {
    let read = json!({"jsonrpc": "2.0", "id": 1, "method": "resources/read", "params": {"uri": "file:///a b"}});
    assert_eq!(
        standard_headers(&read),
        [
            ("Mcp-Method".to_string(), "resources/read".to_string()),
            ("Mcp-Name".to_string(), "file:///a b".to_string())
        ]
    );
    let prompt =
        json!({"jsonrpc": "2.0", "id": 2, "method": "prompts/get", "params": {"name": "总结"}});
    assert_eq!(standard_headers(&prompt)[1].1, header_value("总结"));
    let list = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {}});
    assert_eq!(
        standard_headers(&list),
        [("Mcp-Method".to_string(), "tools/list".to_string())]
    );
    let answer = json!({"jsonrpc": "2.0", "id": 4, "result": {}});
    assert!(standard_headers(&answer).is_empty(), "回应没有方法名");
}

#[test]
fn only_a_json_rpc_error_body_is_handed_back_as_it_is() {
    let id = json!(7);
    let body = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32020,"message":"Header mismatch"}}"#;
    let (line, said) = rpc_error(body, false, Some(&id)).expect("是错误");
    let line: serde_json::Value = serde_json::from_str(&line.expect("有编号的写回")).expect("JSON");
    assert_eq!(line["id"], 7, "编号换成这次的");
    assert_eq!(line["error"]["code"], -32020);
    assert_eq!(said, "-32020 Header mismatch");
    let sse = format!("event: message\ndata: {body}\n\n");
    assert!(rpc_error(&sse, true, Some(&id)).is_some(), "SSE 的照事件拆");
    assert!(
        rpc_error(body, false, None).expect("是错误").0.is_none(),
        "通知不写回"
    );
    assert!(rpc_error("Bad Gateway", false, Some(&id)).is_none());
    assert!(
        rpc_error(r#"{"jsonrpc":"2.0","id":7,"result":{}}"#, false, Some(&id)).is_none(),
        "不是错误的不算"
    );
}

/// 工具参数镜像成请求头（施工 PT-1 中）：标记不合规矩的工具从列表里剔掉、日志里说为什么；合的调用时带上 `Mcp-Param-<名>`，
/// 非 ASCII 的照规范编码，假服务照规范查对得上才答。
#[tokio::test]
async fn marked_parameters_go_into_headers_and_broken_marks_drop_the_tool() {
    for sse in [false, true] {
        let server = HttpServer::modern(sse).await;
        server.header_tools();
        let (log, lines) = log();
        let client = connect(&server, log).await.0.expect("连得上");
        let listed = client.tools().await.expect("列得出");
        let names: Vec<&str> = listed.tools.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(names, ["echo", "region"], "标记写在 items 里的剔掉");
        assert!(
            logged(&lines, "WARN tool broken left out"),
            "{:?}",
            lines.lock().unwrap()
        );
        let called = client
            .call("region", json!({"region": "华东"}))
            .await
            .expect("请求头对得上才答");
        assert_eq!(called.content.len(), 1, "{called:?}");
        let call = server
            .received()
            .into_iter()
            .find(|request| request.body["params"]["name"] == "region")
            .expect("调过");
        assert_eq!(
            call.header("mcp-param-region"),
            Some(header_value("华东").as_str())
        );
    }
}

/// 等到 `server` 记下有人中途断开，最多等 5 秒。
async fn hung_up(server: &HttpServer) -> bool {
    for _ in 0..250 {
        if server.hung_up() > 0 {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

/// 取消就是断开（施工 PT-1 中，规范 2026-07-28「Cancellation」）：客户端丢掉一次调用，转接停掉那个 POST，服务那边看到连接断了；
/// 新版的不另发 `notifications/cancelled`，握过手的老服务照旧发。
#[tokio::test]
async fn a_dropped_call_hangs_up_its_request() {
    for modern in [true, false] {
        let server = if modern {
            HttpServer::modern(false).await
        } else {
            HttpServer::start(false).await
        };
        server.slow(30_000);
        let (log, _) = log();
        let client = connect(&server, log).await.0.expect("连得上");
        let gave_up = tokio::time::timeout(
            Duration::from_millis(300),
            client.call("echo", json!({"text": "hi"})),
        )
        .await;
        assert!(gave_up.is_err(), "等不及丢掉");
        assert!(hung_up(&server).await, "服务看到断开（modern={modern}）");
        // 老服务的通知是另一个 POST，给它一点时间到。
        tokio::time::sleep(Duration::from_millis(200)).await;
        let told = server
            .received()
            .iter()
            .any(|request| request.body["method"] == "notifications/cancelled");
        assert_eq!(told, !modern, "新版不发通知、老版照发（modern={modern}）");
    }
}
