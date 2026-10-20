//! 网址的 MCP 服务（施工 X-32 上，`docs/blueprint/mcp.md`「网址的服务怎么说话」）：个人设置里写 `url` 的服务，核心不起进程，经
//! Streamable HTTP 连上、列出工具、会话里调得到；令牌发成 `Authorization: Bearer`、别的请求头照写的带；`mcp.list` 带 `url`、没有
//! `pid`。`http://` 不是本机地址的不连，停下、日志里说清。

use std::sync::Arc;

use miyu_mcp::testkit::http::HttpServer;
use miyu_session::testkit::{Play, Script};

use crate::support::mcp::*;
use crate::support::*;

#[tokio::test]
async fn a_url_server_is_connected_listed_and_called() {
    let server = HttpServer::start(true).await;
    let home = Home::new();
    home.write(
        "home/alice/settings.toml",
        &format!(
            "[mcp.remote]\nurl = \"{}\"\nbearer = \"t0k\"\nheaders = {{ X-Team = \"core\" }}\nneed_load = false\n",
            server.url
        ),
    );
    let script = Script::new([
        Play::calls(&[("mcp__remote__echo", r#"{"text":"你好"}"#)]),
        Play::Says("好。"),
    ]);
    let core = configured(&home, &script);
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let running = until_listed(&mut client, "remote", |one| one["state"] == "running").await;
    assert_eq!(running["url"], server.url.as_str(), "{running}");
    assert!(
        running.get("pid").is_none(),
        "网址的服务没有进程：{running}"
    );
    assert!(running.get("command").is_none(), "{running}");
    let session = open(&mut client).await;
    client.say("s1", &session, "说一句").await;
    home.until_turns(&session, 1).await;
    assert_eq!(
        results(&home, &session),
        [(
            "mcp__remote__echo".to_string(),
            "你好".to_string(),
            false,
            0
        )]
    );
    let received = server.received();
    assert!(!received.is_empty());
    assert!(
        received
            .iter()
            .all(|request| request.header("authorization") == Some("Bearer t0k")),
        "{received:?}"
    );
    assert!(
        received
            .iter()
            .all(|request| request.header("x-team") == Some("core"))
    );
    assert!(
        received
            .iter()
            .any(|request| request.body["method"] == "tools/call"),
        "调用经网址发过去了"
    );
}

#[tokio::test]
async fn plain_http_to_another_machine_is_not_connected() {
    let home = Home::new();
    home.write(
        "home/alice/settings.toml",
        "[mcp.remote]\nurl = \"http://192.0.2.1/mcp\"\nbearer = \"t0k\"\n",
    );
    let core = configured(&home, &Script::new([]));
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let stopped = until_listed(&mut client, "remote", |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "cannot_start", "{stopped}");
    let said = stopped["stderr"].as_str().unwrap_or_default();
    assert!(said.contains("use https://"), "日志里说清：{stopped}");
}

#[tokio::test]
async fn an_expiring_session_reconnects_without_counting_failures() {
    let server = HttpServer::start(false).await;
    let home = Home::new();
    home.write(
        "home/alice/settings.toml",
        &format!(
            "[mcp.remote]\nurl = \"{}\"\nneed_load = false\n",
            server.url
        ),
    );
    let turn = || {
        [
            Play::calls(&[("mcp__remote__echo", r#"{"text":"在吗"}"#)]),
            Play::Says("好。"),
        ]
    };
    let script = Script::new(turn().into_iter().chain(turn()).chain(turn()));
    let core = configured(&home, &script);
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    until_listed(&mut client, "remote", |one| one["state"] == "running").await;
    let session = open(&mut client).await;
    for n in 1..=3 {
        server.expire();
        client.say(&format!("s{n}"), &session, "说一句").await;
        home.until_turns(&session, n).await;
        let back = until_listed(&mut client, "remote", |one| one["state"] == "running").await;
        assert_eq!(back["failures"], 0, "第 {n} 次过期不算失败：{back}");
    }
    let shakes = server
        .received()
        .iter()
        .filter(|request| request.body["method"] == "initialize")
        .count();
    assert_eq!(shakes, 4, "起来一次，过期三次各重连一次");
}

/// 头经 `mcp.set` 加上网址的服务（施工 X-32 下）：写进个人设置、连上；`mcp.list` 照写的交回 `url`、`bearer`、`headers`，编辑页
/// 照它填，再交回 `mcp.set` 一字不变。
#[tokio::test]
async fn a_url_server_is_set_and_listed_as_written() {
    let server = HttpServer::start(false).await;
    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    let core = configured(&home, &Script::new([]));
    let _watching = core.watch_config().expect("监视得了");
    core.start_extensions();
    let _following = core.follow_home_mcp();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let params = serde_json::json!({
        "name": "remote",
        "url": server.url,
        "bearer": "t0k",
        "headers": {"X-Team": "core"},
    });
    let set = client.call("s1", "mcp.set", params.clone()).await;
    assert_eq!(
        set["result"],
        serde_json::json!({"name": "remote"}),
        "{set}"
    );
    let path = home.root.path().join("home/alice/settings.toml");
    let text = std::fs::read_to_string(&path).expect("写了个人设置");
    let block = format!(
        "\n[mcp.remote]\nurl = \"{}\"\nbearer = \"t0k\"\nheaders = {{ X-Team = \"core\" }}\n",
        server.url
    );
    assert!(text.ends_with(&block), "接在末尾：{text}");
    let running = until_listed(&mut client, "remote", |one| one["state"] == "running").await;
    assert_eq!(running["url"], server.url.as_str(), "{running}");
    assert_eq!(running["bearer"], "t0k", "{running}");
    assert_eq!(
        running["headers"],
        serde_json::json!({"X-Team": "core"}),
        "{running}"
    );
    assert!(
        server
            .received()
            .iter()
            .all(|request| request.header("authorization") == Some("Bearer t0k")),
        "照写的令牌连"
    );

    // 编辑页照列出来的填，原样交回：字一个不变。
    let again = client.call("s2", "mcp.set", params).await;
    assert_eq!(
        again["result"],
        serde_json::json!({"name": "remote"}),
        "{again}"
    );
    assert_eq!(std::fs::read_to_string(&path).expect("还在"), text);
    core.stop_extensions().await;
}

/// 先配服务、后存令牌（施工 X-32 补，GitHub 的远程服务撞出来的）：密钥存了、换了、删了，引用它的、开着的服务停下再起，连续失败
/// 从零数；没引用它的不动；包没开的、带着引用它的服务的，不因此被拉起来。现在只有管理员一个账号、一份密钥文件；多用户那一段照
/// 账号分开。
#[tokio::test]
async fn a_server_follows_the_secret_it_uses() {
    let server = HttpServer::start(false).await;
    server.demand(Some("t1"));
    let other = HttpServer::start(false).await;
    let packaged = HttpServer::start(false).await;
    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    home.write(
        "home/alice/settings.toml",
        &format!(
            "[mcp.remote]\nurl = \"{}\"\nbearer = {{ secret = \"t\" }}\n\n[mcp.other]\nurl = \"{}\"\n",
            server.url, other.url
        ),
    );
    home.write(
        "home/alice/packages/kit/package.toml",
        &format!(
            "[package]\nname = {{ en = \"kit\" }}\n\n[[mcp]]\nname = \"kitted\"\nurl = \"{}\"\nbearer = {{ secret = \"t\" }}\n",
            packaged.url
        ),
    );
    let core = configured(&home, &Script::new([]));
    let _watching = core.watch_config().expect("监视得了");
    core.start_extensions();
    let _following = core.follow_home_mcp();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    until_listed(&mut client, "other", |one| one["state"] == "running").await;
    until_listed(&mut client, "remote", |one| {
        one["failures"]
            .as_u64()
            .is_some_and(|failures| failures > 0)
    })
    .await;

    let set = client
        .call(
            "s1",
            "secret.set",
            serde_json::json!({"name": "t", "value": "t1"}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    until_listed(&mut client, "remote", |one| {
        one["state"] == "running" && one["failures"] == 0
    })
    .await;

    // 在跑的、换了值的也停下再起：照新的令牌连。
    server.demand(Some("t2"));
    client
        .call(
            "s2",
            "secret.set",
            serde_json::json!({"name": "t", "value": "t2"}),
        )
        .await;
    until("照新的令牌重连", || {
        server
            .received()
            .iter()
            .any(|request| request.header("authorization") == Some("Bearer t2"))
    })
    .await;
    until_listed(&mut client, "remote", |one| one["state"] == "running").await;

    // 删了：停下重连，又连不上。
    client
        .call("s3", "secret.delete", serde_json::json!({"name": "t"}))
        .await;
    until_listed(&mut client, "remote", |one| {
        one["failures"]
            .as_u64()
            .is_some_and(|failures| failures > 0)
    })
    .await;

    let shakes = other
        .received()
        .iter()
        .filter(|request| request.body["method"] == "initialize")
        .count();
    assert_eq!(shakes, 1, "没引用这个密钥的不动");
    assert!(
        packaged.received().is_empty(),
        "包没开的不因为密钥变了被拉起来"
    );
    core.stop_extensions().await;
}
