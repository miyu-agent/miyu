//! 登录来的服务（施工 X-42 下）：每个请求现场问令牌；回 `401` 的续一次、照新令牌重发；续不成的、回 `403 insufficient_scope` 的记下
//! 挑战、关掉这一对读写。

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::http::{Credentials, Pending};

/// 现场给令牌的：先给 `now`，被拒了换成 `next`（没有的续不成）；记下续了几次。
struct Rotating {
    now: Mutex<String>,
    next: Option<String>,
    renewed: AtomicUsize,
}

impl Rotating {
    fn new(now: &str, next: Option<&str>) -> Arc<Rotating> {
        Arc::new(Rotating {
            now: Mutex::new(now.to_string()),
            next: next.map(str::to_string),
            renewed: AtomicUsize::new(0),
        })
    }
}

impl Credentials for Rotating {
    fn token(&self) -> Pending<'_> {
        let now = self.now.lock().unwrap().clone();
        Box::pin(async move { Some(now) })
    }

    fn renew<'a>(&'a self, rejected: &'a str) -> Pending<'a> {
        self.renewed.fetch_add(1, Ordering::Relaxed);
        let mut now = self.now.lock().unwrap();
        let renewed = match (&self.next, *now == rejected) {
            (_, false) => Some(now.clone()),
            (Some(next), true) => {
                now.clone_from(next);
                Some(next.clone())
            }
            (None, true) => None,
        };
        Box::pin(async move { renewed })
    }
}

/// 照凭据 `credentials` 连 `server`，不另带请求头。
async fn connect_with(
    server: &HttpServer,
    credentials: Arc<Rotating>,
) -> (Result<Client, Failed>, super::super::Challenged) {
    let opened = open(
        miyu_http::client(miyu_http::Proxy::Off).expect("客户端"),
        server.url.clone(),
        Vec::new(),
        log().0,
        Some(credentials),
    );
    let challenged = opened.challenged.clone();
    let client = Client::connect(opened.read, opened.write, hello(), None, Waits::default()).await;
    (client, challenged)
}

#[tokio::test]
async fn a_rejected_token_is_renewed_once_and_the_request_sent_again() {
    let server = HttpServer::start(false).await;
    server.demand(Some("new"));
    let credentials = Rotating::new("old", Some("new"));
    let client = connect_with(&server, Arc::clone(&credentials))
        .await
        .0
        .expect("续了以后连得上");
    client.tools().await.expect("列得出");
    assert_eq!(credentials.renewed.load(Ordering::Relaxed), 1, "只续一次");
    let tokens: Vec<_> = server
        .received()
        .iter()
        .map(|request| {
            request
                .header("authorization")
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert_eq!(tokens[0], "Bearer old");
    assert!(
        tokens[1..].iter().all(|token| token == "Bearer new"),
        "{tokens:?}"
    );
}

#[tokio::test]
async fn a_token_that_cannot_be_renewed_closes_the_connection_with_the_challenge() {
    let server = HttpServer::start(false).await;
    server.protect("http://127.0.0.1:9", "new");
    let (client, challenged) = connect_with(&server, Rotating::new("old", None)).await;
    assert!(client.is_err(), "连不上");
    let header = challenged.header().expect("记下了挑战");
    assert!(header.contains("resource_metadata"), "{header}");
}

#[tokio::test]
async fn more_scope_closes_the_connection_with_its_challenge() {
    let server = HttpServer::start(false).await;
    server.demand(Some("ok"));
    let (client, challenged) = connect_with(&server, Rotating::new("ok", None)).await;
    let client = client.expect("连得上");
    server.need_scope("files:write");
    let called = tokio::time::timeout(
        Duration::from_secs(5),
        client.call("echo", json!({"text": "hi"})),
    )
    .await
    .expect("当场有结局");
    assert!(called.is_err(), "{called:?}");
    tokio::time::timeout(Duration::from_secs(5), client.closed())
        .await
        .expect("这一对读写关了");
    let header = challenged.header().expect("记下了挑战");
    assert!(
        header.contains("insufficient_scope") && header.contains("files:write"),
        "{header}"
    );
}
