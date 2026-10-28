//! 登录以后（施工 X-42 下，`docs/construction/X-42下-登录的续期和登出.md`）：令牌快过期的请求前续、服务回 `401` 的续一次重发，
//! 续期写回密钥不重起服务（在跑的调用不断），人自己改了那一份的照旧重起；续不成的停在 `needs_login`；服务要更多范围的停在
//! `needs_login`、下次登录的范围并上；`mcp.logout` 删掉那一份；浏览器不在核心那台机器上的把跳到的地址贴回来
//! （`mcp.login_finish`），和本机回调同一处判。远程连进来的人（施工 X-42 下补）照样能登录、贴回来、登出。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_mcp::testkit::http::HttpServer;
use miyu_mcp::testkit::oauth::{AuthServer, browse, redirect_of};
use miyu_session::testkit::{Play, Script};

use crate::mcp_login::{login_url, query, running_with};
use crate::support::mcp::*;
use crate::support::*;

/// 调一次服务的工具 `echo`，再说一句。
fn calling() -> Script {
    Script::new([
        Play::calls(&[("mcp__notes__echo", r#"{"text":"hi"}"#)]),
        Play::Says("好。"),
    ])
}

/// 个人设置写一张要登录的服务 `notes`，核心照剧本 `script` 起好、连好头，登录一次、等到它连上。
async fn logged_in(
    home: &Home,
    server: &HttpServer,
    script: &Script,
) -> (Arc<miyu_endpoint::Core>, Client, Box<dyn std::any::Any>) {
    home.write(
        "config.toml",
        &format!("[mcp.notes]\nurl = \"{}\"\nneed_load = false\n", server.url),
    );
    let (core, mut client, kept) = running_with(home, script).await;
    let waiting = until_listed(&mut client, "notes", |one| one["reason"] == "needs_login").await;
    assert_eq!(waiting.get("logged_in"), None, "没登录的不带：{waiting}");
    let url = login_url(&mut client, "l1").await;
    let (status, page) = browse(&url).await;
    assert_eq!(status, 200, "{page}");
    until_listed(&mut client, "notes", |one| one["state"] == "running").await;
    (core, client, kept)
}

/// 服务收到过几次握手：重起一次多一次。
fn shakes(server: &HttpServer) -> usize {
    server
        .received()
        .iter()
        .filter(|request| request.body["method"] == "initialize")
        .count()
}

/// 服务收到的调工具的请求带的令牌，照先后。
fn called_with(server: &HttpServer) -> Vec<String> {
    server
        .received()
        .iter()
        .filter(|request| request.body["method"] == "tools/call")
        .map(|request| {
            request
                .header("authorization")
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

/// 开一个会话调一次工具，等这一轮说完；交回会话编号。
async fn call_once(home: &Home, client: &mut Client, id: &str) -> String {
    let session = open(client).await;
    client.say(id, &session, "调一下").await;
    home.until_turns(&session, 1).await;
    session
}

/// 会话里那一次调用成没成。
fn called_ok(home: &Home, session: &str) -> bool {
    results(home, session)
        .iter()
        .any(|(name, _, error, _)| name == "mcp__notes__echo" && !error)
}

#[tokio::test]
async fn a_token_about_to_expire_is_renewed_before_the_call_without_a_restart() {
    let auth = AuthServer::start().await;
    // 发出来 4 秒以内还新鲜（核心照整秒比，离到期不到 60 秒的先续）：登录完 3 秒内连上的都不先续，Windows 慢也够（施工 X-42
    // 下修：原来只给一秒，CI 上连的时候就过了线）。
    auth.expire_after(64);
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let script = calling();
    let (_core, mut client, _kept) = logged_in(&home, &server, &script).await;
    let before = shakes(&server);
    // 离到期不到 60 秒了：下一次请求前先续。
    tokio::time::sleep(Duration::from_millis(4200)).await;
    server.protect(&auth.issuer, "access-2");
    let session = call_once(&home, &mut client, "s1").await;
    assert!(called_ok(&home, &session), "{:?}", results(&home, &session));
    assert_eq!(called_with(&server), ["Bearer access-2"]);
    assert_eq!(auth.refreshes().len(), 1, "续了一次");
    assert_eq!(shakes(&server), before, "续期写回密钥不重起服务");
    let secrets = std::fs::read_to_string(home.root.path().join("secrets.toml")).expect("在");
    assert!(secrets.contains("access-2"), "续到的存回去了");
}

#[tokio::test]
async fn a_rejected_token_is_renewed_and_the_call_sent_again() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let script = calling();
    let (_core, mut client, _kept) = logged_in(&home, &server, &script).await;
    server.protect(&auth.issuer, "access-2");
    let session = call_once(&home, &mut client, "s1").await;
    assert!(called_ok(&home, &session), "{:?}", results(&home, &session));
    assert_eq!(
        called_with(&server),
        ["Bearer access-1", "Bearer access-2"],
        "被拒了续一次、重发"
    );
    assert_eq!(auth.refreshes().len(), 1);
}

#[tokio::test]
async fn a_refresh_that_is_refused_stops_at_needs_login() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let script = calling();
    let (_core, mut client, _kept) = logged_in(&home, &server, &script).await;
    auth.refuse_refresh();
    server.protect(&auth.issuer, "access-2");
    call_once(&home, &mut client, "s1").await;
    let stopped = until_listed(&mut client, "notes", |one| one["reason"] == "needs_login").await;
    assert_eq!(stopped["failures"], 0, "{stopped}");
}

#[tokio::test]
async fn a_call_in_flight_outlives_a_refresh_and_a_person_s_change_still_restarts() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("mcp__notes__echo", r#"{"text":"slow"}"#)]),
        Play::calls(&[("mcp__notes__echo", r#"{"text":"fast"}"#)]),
        Play::Says("好。"),
        Play::Says("好。"),
    ]);
    let (_core, mut client, _kept) = logged_in(&home, &server, &script).await;
    let before = shakes(&server);
    // 第一个会话的调用拿着令牌发出去、慢慢答。
    server.slow(2500);
    let first = open(&mut client).await;
    client.say("s1", &first, "慢的").await;
    until(|| called_with(&server).len() == 1).await;
    // 服务改认新的：第二个会话的调用回 401，续一次、写回密钥、重发（施工 X-42 下修：不靠时间，原来过快到期的那条线只有
    // 一秒，Windows 的 CI 上第一次调用就过了线）。
    server.slow(0);
    server.protect(&auth.issuer, "access-2");
    let second = open(&mut client).await;
    client.say("s2", &second, "快的").await;
    home.until_turns(&second, 1).await;
    home.until_turns(&first, 1).await;
    assert!(
        called_ok(&home, &first),
        "在跑的那一次没断：{:?}",
        results(&home, &first)
    );
    assert!(called_ok(&home, &second), "{:?}", results(&home, &second));
    assert_eq!(
        called_with(&server),
        ["Bearer access-1", "Bearer access-1", "Bearer access-2"],
        "第二次被拒了续一次、重发"
    );
    assert_eq!(auth.refreshes().len(), 1);
    assert_eq!(shakes(&server), before, "没重起");
    // 人自己改了那一份：照旧重起。
    let secrets = std::fs::read_to_string(home.root.path().join("secrets.toml")).expect("在");
    let stored: Value = secret_value(&secrets, "mcp-oauth-notes");
    let mut edited = stored.clone();
    edited["scope"] = json!("files:read");
    let set = client
        .call(
            "k1",
            "secret.set",
            json!({"name": "mcp-oauth-notes", "value": edited.to_string()}),
        )
        .await;
    assert_eq!(set["result"]["replaced"], true, "{set}");
    until(|| shakes(&server) > before).await;
}

#[tokio::test]
async fn a_server_wanting_more_scope_stops_and_the_next_login_asks_for_both() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let script = calling();
    let (_core, mut client, _kept) = logged_in(&home, &server, &script).await;
    server.need_scope("files:write");
    call_once(&home, &mut client, "s1").await;
    until_listed(&mut client, "notes", |one| one["reason"] == "needs_login").await;
    let url = login_url(&mut client, "l2").await;
    let scope = query(&url, "scope").unwrap_or_default();
    for wanted in ["files:read", "files:write", "offline_access"] {
        assert!(scope.split(' ').any(|one| one == wanted), "{scope}");
    }
}

#[tokio::test]
async fn logging_out_forgets_the_login() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    let (_core, mut client, _kept) = logged_in(&home, &server, &Script::new([])).await;
    // 登录过的看得出（施工 X-42 下再补）：头照它把「登录」换成「登出」。
    let running = until_listed(&mut client, "notes", |one| one["state"] == "running").await;
    assert_eq!(running["logged_in"], true, "{running}");
    let out = client
        .call("o1", "mcp.logout", json!({"service": "notes"}))
        .await;
    assert_eq!(out["result"], json!({}), "{out}");
    let secrets =
        std::fs::read_to_string(home.root.path().join("secrets.toml")).unwrap_or_default();
    assert!(!secrets.contains("mcp-oauth-notes"), "{secrets}");
    let out = until_listed(&mut client, "notes", |one| one["reason"] == "needs_login").await;
    assert_eq!(out.get("logged_in"), None, "登出了就不带：{out}");
    let again = client
        .call("o2", "mcp.logout", json!({"service": "notes"}))
        .await;
    assert_eq!(again["error"]["data"]["reason"], "not_logged_in", "{again}");
}

#[tokio::test]
async fn a_pasted_callback_is_checked_like_the_local_one() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    home.write(
        "config.toml",
        &format!("[mcp.notes]\nurl = \"{}\"\nneed_load = false\n", server.url),
    );
    let (_core, mut client, _kept) = running_with(&home, &Script::new([])).await;
    until_listed(&mut client, "notes", |one| one["reason"] == "needs_login").await;
    let early = client
        .call(
            "f0",
            "mcp.login_finish",
            json!({"service": "notes", "url": "http://127.0.0.1:1/callback?code=x&state=y"}),
        )
        .await;
    assert_eq!(early["error"]["data"]["reason"], "not_waiting", "{early}");
    let url = login_url(&mut client, "l1").await;
    let back = redirect_of(&url).await.expect("授权页要跳回去");
    let forged = back.replace(&query(&back, "state").expect("有 state"), "forged");
    let refused = client
        .call(
            "f1",
            "mcp.login_finish",
            json!({"service": "notes", "url": forged}),
        )
        .await;
    assert_eq!(
        refused["error"]["data"]["reason"], "oauth_failed",
        "{refused}"
    );
    let taken = client
        .call(
            "f2",
            "mcp.login_finish",
            json!({"service": "notes", "url": back}),
        )
        .await;
    assert_eq!(
        taken["result"],
        json!({}),
        "那一次登录还在等，贴对的照收：{taken}"
    );
    until_listed(&mut client, "notes", |one| one["state"] == "running").await;
}

/// 等到 `ready` 成立，最多十秒。
async fn until(ready: impl Fn() -> bool) {
    for _ in 0..200 {
        if ready() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("十秒内没等到");
}

/// 密钥文件 `text` 里叫 `name` 的那一份，读成 JSON。
fn secret_value(text: &str, name: &str) -> Value {
    let document: toml_edit::DocumentMut = text.parse().expect("密钥文件是 TOML");
    let value = document[name].as_str().expect("有这一份");
    serde_json::from_str(value).expect("是 JSON")
}

/// 网页软件替远程的人开的一条连接（施工 Y-1 下上）：握手之前先说一句来自哪个地址。
async fn remote(core: &Arc<miyu_endpoint::Core>) -> Client {
    let mut client = Client::connect(Arc::clone(core));
    let notice =
        json!({"jsonrpc": "2.0", "method": "connection.remote", "params": {"from": "203.0.113.9"}});
    client.line(&notice.to_string()).await;
    client.hello().await;
    client
}

#[tokio::test]
async fn a_remote_person_logs_in_by_pasting_the_callback_back_and_logs_out() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let home = Home::new();
    home.write(
        "config.toml",
        &format!("[mcp.notes]\nurl = \"{}\"\nneed_load = false\n", server.url),
    );
    let (core, mut local, _kept) = running_with(&home, &Script::new([])).await;
    until_listed(&mut local, "notes", |one| one["reason"] == "needs_login").await;
    let mut far = remote(&core).await;
    let url = login_url(&mut far, "r1").await;
    let back = redirect_of(&url).await.expect("授权页要跳回去");
    let taken = far
        .call(
            "r2",
            "mcp.login_finish",
            json!({"service": "notes", "url": back}),
        )
        .await;
    assert_eq!(taken["result"], json!({}), "{taken}");
    until_listed(&mut local, "notes", |one| one["state"] == "running").await;
    let out = far
        .call("r3", "mcp.logout", json!({"service": "notes"}))
        .await;
    assert_eq!(out["result"], json!({}), "{out}");
}
