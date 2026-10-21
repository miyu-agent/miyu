//! 网址的服务登录的运行日志（施工 X-42 上，`docs/construction/X-42上-网址的服务登录.md`）：真核心登录一遍（假的要登录的服务、
//! 假授权服务器），整份运行日志（`TRACE`）、系统日志里搜不到令牌、`refresh_token`、授权码、`code_verifier`、`state`；只有
//! `mcp oauth login started`、`mcp oauth logged in` 这几行，带服务名。
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
        "home/alice/settings.toml",
        &format!("[mcp.notes]\nurl = \"{}\"\nneed_load = false\n", server.url),
    );
    home.root.prepare_home(&alice()).expect("建得了家目录");
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
    let log = memory.lines().join("\n");
    assert!(
        log.contains("mcp oauth login started service=notes"),
        "{log}"
    );
    assert!(log.contains("mcp oauth logged in service=notes"), "{log}");
    let verifiers = auth.verifiers();
    assert_eq!(verifiers.len(), 1, "换过一次令牌");
    let mut secrets = vec![
        "access-1".to_string(),
        "refresh-1".to_string(),
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
        std::fs::read_to_string(home.root.path().join("system/journal.jsonl")).unwrap_or_default();
    for secret in &secrets {
        assert!(!journal.contains(secret.as_str()), "系统日志里有 {secret}");
    }
    core.stop_extensions().await;
}
