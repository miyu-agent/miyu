//! 网址的服务登录（施工 X-42 上，`docs/construction/X-42上-网址的服务登录.md`）：真核心连假的要登录的服务（`HttpServer::protect`），
//! 假授权服务器（`AuthServer`），测试自己当浏览器。回 `401` 的停在 `needs_login`、不算失败；`mcp.login` 交回带 PKCE、`state`、
//! `resource` 的授权页，点了同意以后令牌存进密钥文件（0600）、服务自己连上、请求带 `Bearer`；`state` 对不上的不认；表里写了令牌的
//! 不走登录；没有的服务 `unknown_server`；第二次登录绑同一个端口；删了服务，存的那一份跟着清掉。

use std::sync::Arc;

use serde_json::json;

use miyu_mcp::testkit::http::HttpServer;
use miyu_mcp::testkit::oauth::{AuthServer, browse};
use miyu_session::testkit::Script;

use crate::support::mcp::*;
use crate::support::*;

/// 一份核心，看着配置、起了扩展、跟着配置（密钥变了重起、删了服务清掉都在这一路上）；交回核心、连好的头，和要留着的两样。
async fn running(home: &Home) -> (Arc<miyu_endpoint::Core>, Client, Box<dyn std::any::Any>) {
    running_with(home, &Script::new([])).await
}

/// 同 [`running`]，模型照剧本 `script` 答（施工 X-42 下：会话里调服务的工具）。
pub(crate) async fn running_with(
    home: &Home,
    script: &Script,
) -> (Arc<miyu_endpoint::Core>, Client, Box<dyn std::any::Any>) {
    home.root.prepare_workspace().expect("建得了家目录");
    let core = configured(home, script);
    let watching = core.watch_config().expect("监视得了");
    core.start_extensions();
    let following = core.follow_home_mcp();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    (core, client, Box::new((watching, following)))
}

/// 配置写一张要登录的服务 `notes`，核心起好、连好头，等到它停在 `needs_login`。
async fn waiting_for_login(
    home: &Home,
    server: &HttpServer,
) -> (Arc<miyu_endpoint::Core>, Client, Box<dyn std::any::Any>) {
    home.write(
        "config.toml",
        &format!("[mcp.notes]\nurl = \"{}\"\nneed_load = false\n", server.url),
    );
    let (core, mut client, kept) = running(home).await;
    let stopped = until_listed(&mut client, "notes", |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "needs_login", "{stopped}");
    assert_eq!(stopped["failures"], 0, "要登录不算失败：{stopped}");
    (core, client, kept)
}

/// 发 `mcp.login`，交回授权页的网址。
pub(crate) async fn login_url(client: &mut Client, id: &str) -> String {
    let asked = client
        .call(id, "mcp.login", json!({"service": "notes"}))
        .await;
    asked["result"]["url"]
        .as_str()
        .unwrap_or_else(|| panic!("交回授权页：{asked}"))
        .to_string()
}

/// 授权页网址里 `name` 那一格。
pub(crate) fn query(url: &str, name: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()?
        .query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}

#[tokio::test]
async fn a_server_wanting_a_login_connects_after_the_person_agrees() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let (_core, mut client, _kept) = waiting_for_login(&home, &server).await;
    let url = login_url(&mut client, "l1").await;
    assert_eq!(
        query(&url, "code_challenge_method").as_deref(),
        Some("S256")
    );
    assert!(query(&url, "state").is_some_and(|state| state.len() == 43));
    assert_eq!(
        query(&url, "resource").as_deref(),
        Some(server.url.as_str())
    );
    assert_eq!(
        query(&url, "scope").as_deref(),
        Some("files:read offline_access")
    );
    let (status, page) = browse(&url).await;
    assert_eq!(status, 200, "{page}");
    assert!(page.contains("登录好了"), "照连接的语言：{page}");
    let running = until_listed(&mut client, "notes", |one| one["state"] == "running").await;
    assert_eq!(running["failures"], 0, "{running}");
    assert!(
        server
            .received()
            .iter()
            .any(|request| request.header("authorization") == Some("Bearer access-1")),
        "连的时候带上换来的令牌"
    );
    let secrets = home.root.path().join("secrets.toml");
    let written = std::fs::read_to_string(&secrets).expect("密钥文件在");
    assert!(written.contains("mcp-oauth-notes"), "{written}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&secrets)
            .expect("在")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "只有自己读得了");
    }
    let listed = client.call("s1", "secret.list", json!({})).await;
    assert!(
        !listed.to_string().contains("access-1"),
        "协议上只有名字：{listed}"
    );
}

#[tokio::test]
async fn a_callback_with_another_state_is_not_taken() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let (_core, mut client, _kept) = waiting_for_login(&home, &server).await;
    let url = login_url(&mut client, "l1").await;
    let redirect = query(&url, "redirect_uri").expect("有回调地址");
    let forged = format!("{redirect}?code=stolen&state=not-the-one");
    let (status, page) = browse(&forged).await;
    assert_eq!(status, 400, "{page}");
    assert!(page.contains("登录没成"), "{page}");
    let (status, _) = browse(&url).await;
    assert_eq!(status, 200, "对的那一次照样接着等得到");
    until_listed(&mut client, "notes", |one| one["state"] == "running").await;
}

#[tokio::test]
async fn a_second_login_binds_the_same_port_and_removing_the_server_clears_it() {
    let auth = AuthServer::exact_port().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let (core, mut client, _kept) = waiting_for_login(&home, &server).await;
    let first = login_url(&mut client, "l1").await;
    let (status, _) = browse(&first).await;
    assert_eq!(status, 200);
    until_listed(&mut client, "notes", |one| one["state"] == "running").await;
    let second = login_url(&mut client, "l2").await;
    assert_eq!(
        query(&second, "redirect_uri"),
        query(&first, "redirect_uri"),
        "照服务记住的端口"
    );
    assert_eq!(auth.registrations(), 1, "同一个客户端，不重新注册");
    let (status, _) = browse(&second).await;
    assert_eq!(status, 200, "要精确端口的授权服务器也认");
    home.write("config.toml", "");
    let secrets = home.root.path().join("secrets.toml");
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::fs::read_to_string(&secrets)
        .unwrap_or_default()
        .contains("mcp-oauth-notes")
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "删了服务，存的那一份清掉"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    drop(core);
}

#[tokio::test]
async fn only_a_listed_url_server_without_its_own_token_logs_in() {
    let server = HttpServer::start(false).await;
    let home = Home::new();
    home.write(
        "config.toml",
        &format!("[mcp.notes]\nurl = \"{}\"\nbearer = \"t0k\"\n", server.url),
    );
    let (_core, mut client, _kept) = running(&home).await;
    until_listed(&mut client, "notes", |one| one["state"] == "running").await;
    let own = client
        .call("l1", "mcp.login", json!({"service": "notes"}))
        .await;
    assert_eq!(
        reason(&own),
        Some("bad_params"),
        "写了令牌的不走登录：{own}"
    );
    let none = client
        .call("l2", "mcp.login", json!({"service": "nobody"}))
        .await;
    assert_eq!(reason(&none), Some("unknown_server"), "{none}");
}
