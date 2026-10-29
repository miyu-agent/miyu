//! 配置里的 MCP 服务给头的（施工 F-9 补，`docs/blueprint/protocol.md` 的 `mcp.*`）：软件包列表上被盖住的服务、全被盖住的
//! 包带 `covered`；`mcp.list` 列出配置里的服务和它盖住了谁，`mcp.restart` 重启一个、没有的拒；订阅软件包列表的连接收
//! `mcp.changed`，删掉的 `server` 是 `null`，不推叫 `~home` 的包。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::mcp::*;
use crate::support::*;

#[tokio::test]
async fn covered_services_are_marked_and_settings_servers_are_listed_and_restarted() {
    let home = Home::new();
    let (theirs, record) = recording(&home, "package");
    bundle(&home, "kit", "probe", &[&record]);
    // 带两个服务、只被盖住一个的包：包那一项不带 `covered`。
    home.write(
        "packages/pair/package.toml",
        &format!(
            "[package]\nname = {{ en = \"pair\" }}\n\n[[mcp]]\nname = \"lone\"\ncommand = ['{SERVER}']\n\n[[mcp]]\nname = \"spare\"\ncommand = ['{SERVER}']\n"
        ),
    );
    let (ours, record) = recording(&home, "settings");
    home.write(
        "config.toml",
        &format!("{}\n[mcp.lone]\ncommand = ['{SERVER}']\n", mine(&record)),
    );
    let core = configured(&home, &Script::new([]));
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "kit").await;
    until_line(&ours, "tools/list").await;
    let pair = entry(&mut client, "pair").await;
    assert!(pair.get("covered").is_none(), "只被盖住一个：{pair}");
    assert_eq!(pair["mcp"][0]["covered"], true, "{pair}");
    assert!(pair["mcp"][1].get("covered").is_none(), "{pair}");

    let kit = entry(&mut client, "kit").await;
    assert_eq!(kit["covered"], true, "服务全被盖住的包：{kit}");
    assert_eq!(kit["status"], "off", "{kit}");
    assert_eq!(
        (
            kit["mcp"][0]["covered"].clone(),
            kit["mcp"][0]["state"].clone()
        ),
        (json!(true), json!("off")),
        "{kit}"
    );
    assert!(!theirs.exists());

    let first = until_listed(&mut client, "probe", |one| one["state"] == "running").await;
    assert_eq!(first["covers"], json!(["kit"]), "{first}");
    assert_eq!(first["failures"], 0, "{first}");
    assert!(first["command"].is_array(), "{first}");
    assert_eq!(first["venues"], json!(["local"]), "{first}");
    let pid = first["pid"].as_u64().expect("在跑的带进程号");
    let restarted = client
        .call("r", "mcp.restart", json!({"name": "probe"}))
        .await;
    assert_eq!(restarted["result"]["name"], "probe", "{restarted}");
    let again = until_listed(&mut client, "probe", |one| {
        one["state"] == "running" && one["pid"].as_u64() != Some(pid)
    })
    .await;
    assert_eq!(again["failures"], 0, "重启从零数：{again}");
    let unknown = client
        .call("u", "mcp.restart", json!({"name": "nope"}))
        .await;
    assert_eq!(
        unknown["error"]["data"]["reason"], "unknown_server",
        "{unknown}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn subscribers_hear_mcp_changed_and_never_a_home_package() {
    let home = Home::new();
    home.root.prepare_workspace().expect("建得了家目录");
    home.write("config.toml", "[ui]\nlanguage = \"zh\"\n");
    let core = configured(&home, &Script::new([]));
    let _watching = core.watch_config().expect("监视得了");
    core.start_extensions();
    let _following = core.follow_home_mcp();
    let mut watcher = Client::connect(Arc::clone(&core));
    watcher.hello().await;
    let subscribed = watcher
        .call("w", "subscribe", json!({"stream": "packages"}))
        .await;
    assert!(subscribed["result"]["packages"].is_array(), "{subscribed}");

    let (_, record) = recording(&home, "settings");
    home.write(
        "config.toml",
        &format!("[ui]\nlanguage = \"zh\"\n\n{}", mine(&record)),
    );
    let running = until_mcp(&mut watcher, |params| {
        params["server"]["state"] == "running"
    })
    .await;
    assert_eq!(running["name"], "probe", "{running}");
    home.write("config.toml", "[ui]\nlanguage = \"zh\"\n");
    let gone = until_mcp(&mut watcher, |params| params["server"].is_null()).await;
    assert_eq!(gone["name"], "probe", "删掉的推 null：{gone}");
    core.stop_extensions().await;
}

/// 读推送，直到一条 `mcp.changed` 合 `wanted`，交回它的 `params`；途中不许有叫 `~home` 的 `package.changed`。最多 60 秒。
async fn until_mcp(watcher: &mut Client, wanted: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let pushed = watcher.next_within(left).await.expect("六十秒内推了");
        assert_ne!(
            pushed["params"]["package"], "~home",
            "个人设置里的服务不当包推：{pushed}"
        );
        if pushed["method"] == "mcp.changed" && wanted(&pushed["params"]) {
            return pushed["params"].clone();
        }
    }
}
