//! 网址的服务登录的运行日志（施工 X-42 上，`docs/construction/X-42上-网址的服务登录.md`）：真核心登录一遍（假的要登录的服务、
//! 假授权服务器），整份运行日志（`TRACE`）、系统日志里搜不到令牌、`refresh_token`、授权码、`code_verifier`、`state`；只有
//! `mcp oauth login started`、`mcp oauth logged in` 这几行，带服务名。续期这一路（施工 X-42 下）同样：续成了的、续不成的各一行，
//! 续不成的只写 OAuth 的 `error` 码，新换来的令牌、`refresh_token` 也搜不到。
//!
//! 全局装一个写进内存的订阅者：全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use std::sync::Arc;

use serde_json::json;

use miyu_log::{LevelFilter, Memory};
use miyu_mcp::testkit::http::HttpServer;
use miyu_mcp::testkit::oauth::{AuthServer, browse};
use miyu_session::testkit::Script;

use support::mcp::*;
use support::*;

#[tokio::test]
async fn logging_in_never_writes_a_token_a_code_or_a_verifier() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::TRACE,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    home.write(
        "config.toml",
        &format!("[mcp.notes]\nurl = \"{}\"\nneed_load = false\n", server.url),
    );
    home.root.prepare_workspace().expect("建得了家目录");
    let core = configured(&home, &Script::new([]));
    let _watching = core.watch_config().expect("监视得了");
    core.start_extensions();
    let _following = core.follow_home_mcp();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    until_listed(&mut client, "notes", |one| one["reason"] == "needs_login").await;
    let asked = client
        .call("l1", "mcp.login", json!({"service": "notes"}))
        .await;
    let url = asked["result"]["url"]
        .as_str()
        .expect("交回授权页")
        .to_string();
    let state = url::Url::parse(&url)
        .expect("是网址")
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .expect("有 state");
    let (status, _) = browse(&url).await;
    assert_eq!(status, 200);
    until_listed(&mut client, "notes", |one| one["state"] == "running").await;
    // 续期（施工 X-42 下）：服务只认新的了，重起一次，连的时候回 401、续一次、连上；再拒绝续、重起一次，停在 needs_login。
    server.protect(&auth.issuer, "access-2");
    client
        .call("r1", "mcp.restart", json!({"name": "notes"}))
        .await;
    until(|| auth.refreshes().len() == 1).await;
    until_listed(&mut client, "notes", |one| one["state"] == "running").await;
    auth.refuse_refresh();
    server.protect(&auth.issuer, "access-3");
    client
        .call("r2", "mcp.restart", json!({"name": "notes"}))
        .await;
    until_listed(&mut client, "notes", |one| one["reason"] == "needs_login").await;
    let log = memory.lines().join("\n");
    assert!(log.contains("mcp oauth refreshed service=notes"), "{log}");
    assert!(
        log.contains("mcp oauth refresh failed service=notes error=invalid_grant"),
        "{log}"
    );
    assert!(
        log.contains("mcp oauth login started service=notes"),
        "{log}"
    );
    assert!(log.contains("mcp oauth logged in service=notes"), "{log}");
    // 续期的请求不带 `code_verifier`，假授权服务器记下的是空的：只看换令牌的那一个。
    let verifiers: Vec<String> = auth
        .verifiers()
        .into_iter()
        .filter(|verifier| !verifier.is_empty())
        .collect();
    assert_eq!(verifiers.len(), 1, "换过一次令牌");
    let mut secrets = vec![
        "access-1".to_string(),
        "refresh-1".to_string(),
        "access-2".to_string(),
        "refresh-2".to_string(),
        "code-1".to_string(),
        state,
    ];
    secrets.extend(verifiers);
    for secret in &secrets {
        assert!(
            !log.contains(secret.as_str()),
            "运行日志里有 {secret}：{log}"
        );
    }
    let journal =
        std::fs::read_to_string(home.root.path().join("journal.jsonl")).unwrap_or_default();
    for secret in &secrets {
        assert!(!journal.contains(secret.as_str()), "系统日志里有 {secret}");
    }
    core.stop_extensions().await;
}

/// 等到 `ready` 成立，最多十秒。
async fn until(ready: impl Fn() -> bool) {
    for _ in 0..200 {
        if ready() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("十秒内没等到");
}
