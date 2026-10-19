//! 头改个人设置里的 MCP 服务（施工 X-26，`docs/blueprint/mcp.md`「个人设置里的一张表」第 2 条）：`mcp.set` 加上、整条换掉，
//! 写进个人设置、别的字不动，正在跑的服务照新写的换；写错的、随包来的拒，什么都不写；`mcp.remove` 删掉、没有的拒。
//! `mcp.list` 知道工具的带 `tools`：起好列完的、照缓存先登记的都算，没起过、没缓存的不带。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::mcp::*;
use crate::support::*;

/// 个人设置的字。
fn settings(home: &Home) -> String {
    std::fs::read_to_string(home.root.path().join("home/alice/settings.toml")).unwrap_or_default()
}

/// `mcp.list` 的服务名。
async fn names(client: &mut Client) -> Vec<String> {
    let reply = client.call("l", "mcp.list", json!({})).await;
    reply["result"]["servers"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|one| one["name"].as_str().map(str::to_string))
        .collect()
}

/// 拒绝的原因码。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

#[tokio::test]
async fn a_set_server_is_written_followed_replaced_and_removed() {
    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    let before = "[ui]\nlanguage = \"zh\" # 中文\n";
    home.write("home/alice/settings.toml", before);
    let core = configured(&home, &Script::new([]));
    let _watching = core.watch_config().expect("监视得了");
    core.start_extensions();
    let _following = core.follow_home_mcp();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let (_, record) = recording(&home, "settings");
    let set = client
        .call(
            "s1",
            "mcp.set",
            json!({
                "name": "probe",
                "command": SERVER,
                "args": [record],
                "env": {"MIYU_MCP_PROBE": "mine"},
                "timeout_ms": 1000,
            }),
        )
        .await;
    assert_eq!(set["result"], json!({"name": "probe"}), "{set}");
    let text = settings(&home);
    assert!(text.starts_with(before), "别的字不动：{text}");
    assert!(
        text.contains("[mcp.probe]\n") && text.contains("env = { MIYU_MCP_PROBE = \"mine\" }"),
        "{text}"
    );
    let first = until_listed(&mut client, "probe", |one| one["state"] == "running").await;
    let pid = first["pid"].as_u64().expect("在跑的带进程号");

    // 整条换掉：没交的格子不留，正在跑的照新写的重起。
    let again = client
        .call(
            "s2",
            "mcp.set",
            json!({
                "name": "probe",
                "command": SERVER,
                "args": ["legacy", record],
                "venues": ["local", "private"],
            }),
        )
        .await;
    assert_eq!(again["result"], json!({"name": "probe"}), "{again}");
    let text = settings(&home);
    assert_eq!(text.matches("[mcp.probe]").count(), 1, "{text}");
    assert!(!text.contains("MIYU_MCP_PROBE"), "{text}");
    assert!(text.contains("venues = [\"local\", \"private\"]"), "{text}");
    until_listed(&mut client, "probe", |one| {
        one["venues"] == json!(["local", "private"])
            && one["state"] == "running"
            && one["pid"].as_u64() != Some(pid)
    })
    .await;

    let removed = client
        .call("r1", "mcp.remove", json!({"name": "probe"}))
        .await;
    assert_eq!(removed["result"], json!({}), "{removed}");
    assert_eq!(settings(&home), before, "删得干干净净");
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    while !names(&mut client).await.is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "删掉的还在列着");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let gone = client
        .call("r2", "mcp.remove", json!({"name": "probe"}))
        .await;
    assert_eq!(reason(&gone), "unknown_server", "{gone}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn bad_servers_and_package_ones_are_refused_and_nothing_is_written() {
    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    bundle(&home, "kit", "carried", &[]);
    let before = "[ui]\nlanguage = \"zh\"\n";
    home.write("home/alice/settings.toml", before);
    let core = configured(&home, &Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    for (wrong, params) in [
        ("名字", json!({"name": "Bad_Name", "command": "x"})),
        ("命令是空的", json!({"name": "ok", "command": ""})),
        (
            "会话种类",
            json!({"name": "ok", "command": "x", "venues": ["nowhere"]}),
        ),
        (
            "时限",
            json!({"name": "ok", "command": "x", "timeout_ms": 5}),
        ),
        (
            "环境变量",
            json!({"name": "ok", "command": "x", "env": {"A": 3}}),
        ),
    ] {
        let reply = client.call("s", "mcp.set", params).await;
        assert_eq!(reason(&reply), "mcp_invalid", "{wrong}：{reply}");
        assert!(
            reply["error"]["data"]["problem"]
                .as_str()
                .is_some_and(|problem| problem.starts_with("MCP 服务的写法不对")),
            "照连接的语言说：{reply}"
        );
    }
    let unknown = client
        .call(
            "s",
            "mcp.set",
            json!({"name": "ok", "command": "x", "shell": true}),
        )
        .await;
    assert_eq!(reason(&unknown), "bad_params", "{unknown}");
    let carried = client
        .call("s", "mcp.set", json!({"name": "carried", "command": "x"}))
        .await;
    assert_eq!(reason(&carried), "server_from_package", "{carried}");
    let missing = client
        .call("r", "mcp.remove", json!({"name": "carried"}))
        .await;
    assert_eq!(reason(&missing), "unknown_server", "{missing}");
    assert_eq!(settings(&home), before, "拒了的什么都不写");

    // 个人设置里已经写了的同名服务照常换：它本来就盖住了包里的。
    home.write(
        "home/alice/settings.toml",
        &format!("{before}\n[mcp.carried]\ncommand = [\"old\"]\n"),
    );
    let replaced = client
        .call("s", "mcp.set", json!({"name": "carried", "command": "new"}))
        .await;
    assert_eq!(replaced["result"], json!({"name": "carried"}), "{replaced}");
    assert_eq!(
        settings(&home),
        format!("{before}\n[mcp.carried]\ncommand = [\"new\"]\n")
    );
}

#[tokio::test]
async fn listed_servers_carry_their_tools_once_known() {
    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    let (_, record) = recording(&home, "settings");
    let flag = home.work.join("probe-down");
    let exit_if = format!("exit-if:{}", flag.display());
    home.write(
        "home/alice/settings.toml",
        &format!(
            "[mcp.probe]\ncommand = ['{SERVER}', '{exit_if}', '{record}']\ntimeout_ms = 1000\n\n[mcp.never]\ncommand = ['{SERVER}', 'exit:3']\n"
        ),
    );
    let core = configured(&home, &Script::new([]));
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let up = until_listed(&mut client, "probe", |one| one["tools"].is_array()).await;
    assert!(
        up["tools"]
            .as_array()
            .is_some_and(|tools| tools.contains(&json!("mcp__probe__echo"))),
        "{up}"
    );
    let never = until_listed(&mut client, "never", |one| one["state"] == "stopped").await;
    assert!(
        never.get("tools").is_none(),
        "没起过、没缓存的不带：{never}"
    );
    core.stop_extensions().await;
    drop(client);

    // 服务起不来了：照缓存先登记的工具照样列着。
    std::fs::write(&flag, b"").expect("写得进");
    let down = configured(&home, &Script::new([]));
    down.start_extensions();
    let mut client = Client::connect(Arc::clone(&down));
    client.hello().await;
    let cached = listed(&mut client, "probe").await;
    assert_ne!(cached["state"], "running", "{cached}");
    assert!(
        cached["tools"]
            .as_array()
            .is_some_and(|tools| tools.contains(&json!("mcp__probe__echo"))),
        "{cached}"
    );
    down.stop_extensions().await;
}
