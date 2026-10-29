//! MCP 的功能和状态（施工 X-4，`docs/construction/X-4-MCP的功能和状态.md`）：软件包列表上每个服务一格带自己的状态（在跑的带
//! 进程号，停下的带原因和标准错误的最后几行），变了推一声；`package.restart` 重启带的服务，关着的、没有能重启的拒；预设里一个
//! 服务一个功能，整个关掉、单件关掉照常换工具面，`preset.get` 列出它的工具。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

/// 一直读推送，读到包 `id` 的一项满足 `wanted` 为止，交回那一项。
async fn until_pushed(client: &mut Client, id: &str, wanted: impl Fn(&Value) -> bool) -> Value {
    loop {
        let next = client.next().await.expect("没断开");
        let entry = &next["params"]["entry"];
        if next["method"] == "package.changed" && next["params"]["package"] == id && wanted(entry) {
            return entry.clone();
        }
    }
}

/// 包 `id` 唯一的服务这时的那一格。
async fn service(client: &mut Client, id: &str) -> Value {
    entry(client, id).await["mcp"][0].clone()
}

#[tokio::test]
async fn each_service_shows_its_state_and_a_restart_brings_a_stopped_one_back() {
    let home = Home::new();
    let block = home.work.join("block");
    std::fs::write(&block, "").expect("写得了");
    let blocked = format!("exit-if:{}", block.display());
    bundle(&home, "probe", "probe", &["stray:cannot start", &blocked]);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let mut watcher = Client::connect(Arc::clone(&core));
    watcher.hello().await;
    let subscribed = watcher
        .call("w", "subscribe", json!({"stream": "packages"}))
        .await;
    assert!(subscribed["result"]["packages"].is_array(), "{subscribed}");
    let off = service(&mut client, "probe").await;
    assert_eq!(
        (
            off["name"].clone(),
            off["state"].clone(),
            off["failures"].clone()
        ),
        (json!("probe"), json!("off"), json!(0)),
        "{off}"
    );

    enable(&mut client, "probe").await;
    let pushed = until_pushed(&mut watcher, "probe", |entry| {
        entry["mcp"][0]["state"] == "stopped"
    })
    .await;
    let stopped = &pushed["mcp"][0];
    assert_eq!(stopped["failures"], 5, "试了 5 次停下：{stopped}");
    assert!(stopped["reason"].is_string(), "{stopped}");
    let stderr = stopped["stderr"].as_str().unwrap_or_default();
    assert!(stderr.contains("cannot start"), "{stopped}");

    std::fs::remove_file(&block).expect("删得了");
    let restarted = client
        .call("r", "package.restart", json!({"package": "probe"}))
        .await;
    assert_eq!(restarted["result"]["package"], "probe", "{restarted}");
    let running = until_status(&mut client, "probe", "running").await;
    let fresh = &running["mcp"][0];
    assert_eq!(fresh["state"], "running", "{fresh}");
    assert!(fresh["pid"].is_u64(), "在跑的带进程号：{fresh}");
    assert_eq!(fresh["failures"], 0, "重启从零数：{fresh}");
    assert!(fresh.get("stderr").is_none(), "{fresh}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_restart_starts_the_service_afresh_and_its_tools_keep_working() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &[]);
    let script = Script::new([
        Play::calls(&[("mcp__probe__echo", r#"{"text":"a"}"#)]),
        Play::Says("好。"),
        Play::calls(&[("mcp__probe__echo", r#"{"text":"b"}"#)]),
        Play::Says("好。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    let before = service(&mut client, "probe").await["pid"].clone();
    let session = open(&mut client).await;
    client.say("s1", &session, "一").await;
    home.until_turns(&session, 1).await;

    client
        .call("r", "package.restart", json!({"package": "probe"}))
        .await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let now = service(&mut client, "probe").await;
        if now["state"] == "running" && now["pid"] != before {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到新进程：{now}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    client.say("s2", &session, "二").await;
    home.until_turns(&session, 2).await;
    let texts: Vec<String> = results(&home, &session)
        .into_iter()
        .map(|(_, text, error, _)| format!("{text}{}", if error { " !" } else { "" }))
        .collect();
    assert_eq!(texts, ["a", "b"]);
    core.stop_extensions().await;
}

#[tokio::test]
async fn restarting_an_off_or_unrestartable_package_is_refused() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &[]);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    for (package, wanted) in [
        ("probe", "package_off"),
        ("miyu-base-memory", "not_restartable"),
        ("nope", "unknown_package"),
    ] {
        let reply = client
            .call("r", "package.restart", json!({"package": package}))
            .await;
        assert_eq!(reason(&reply), Some(wanted), "{package}：{reply}");
    }
    core.stop_extensions().await;
}

/// `preset.get` 回应里编号是 `id` 的功能。
fn feature<'a>(reply: &'a Value, id: &str) -> &'a Value {
    reply["result"]["features"]
        .as_array()
        .and_then(|features| features.iter().find(|feature| feature["id"] == id))
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

#[tokio::test]
async fn a_preset_turns_a_service_off_whole_or_one_tool_at_a_time() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &[]);
    home.write(
        "presets/nomcp.toml",
        "[preset]\nname = { en = \"No MCP\" }\n\n[features]\nmcp-probe = false\n",
    );
    home.write(
        "presets/noecho.toml",
        "[preset]\nname = { en = \"No echo\" }\n\n[tools]\nmcp__probe__echo = false\n",
    );
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;

    let listed = client
        .call("g", "preset.get", json!({"preset": "noecho"}))
        .await;
    let probe = feature(&listed, "mcp-probe");
    assert_eq!(probe["on"], true, "{probe}");
    let tools = probe["tools"].as_array().expect("列出工具");
    let echo = tools.iter().find(|tool| tool["name"] == "mcp__probe__echo");
    assert_eq!(
        echo.map(|tool| tool["on"].clone()),
        Some(json!(false)),
        "{probe}"
    );
    let env = tools.iter().find(|tool| tool["name"] == "mcp__probe__env");
    assert_eq!(
        env.map(|tool| tool["on"].clone()),
        Some(json!(true)),
        "{probe}"
    );
    // 给人看的名字（施工 X-27 再补）：服务给了 `title` 的照它，没给的去掉 `mcp__probe__`。
    assert_eq!(
        (
            echo.map(|tool| tool["label"].clone()),
            env.map(|tool| tool["label"].clone())
        ),
        (Some(json!("Echo back")), Some(json!("env"))),
        "{probe}"
    );

    let mut faces = Vec::new();
    for (n, preset) in ["full", "nomcp", "noecho"].into_iter().enumerate() {
        let session = client
            .create_with(
                &format!("c{n}"),
                json!({"cwd": "~", "persona": "engineer", "preset": preset}),
            )
            .await;
        client.say(&format!("s{n}"), &session, "在吗").await;
        home.until_turns(&session, 1).await;
        faces.push(face(&script, n));
    }
    assert!(
        faces[0].contains(&"mcp__probe__echo".to_string()),
        "{faces:?}"
    );
    assert!(faces[1].is_empty(), "整个关掉的一件都没有：{faces:?}");
    assert!(
        !faces[2].contains(&"mcp__probe__echo".to_string()),
        "{faces:?}"
    );
    assert_eq!(
        faces[2].len(),
        faces[0].len() - 1,
        "只少关掉的那一件：{faces:?}"
    );
    let system = &script.requests()[1].1.system;
    assert!(!system.contains("Installed but off"), "{system}");
    core.stop_extensions().await;
}

/// Windows 上 `npx`、`uvx` 这类是 `.cmd`（施工 X-5）：包的 `bin/` 里放一个 `.cmd` 转给测试用的服务，命令只写名字，照样起得来、
/// 握得上手。
#[cfg(windows)]
#[tokio::test]
async fn on_windows_a_cmd_wrapper_starts_the_service() {
    let home = Home::new();
    home.write(
        "packages/wrapped/package.toml",
        "[package]\nname = { en = \"wrapped\" }\n\n[[mcp]]\nname = \"wrapped\"\ncommand = [\"probe\"]\n",
    );
    home.write(
        "packages/wrapped/bin/probe.cmd",
        &format!("@\"{SERVER}\" %*\r\n"),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "wrapped").await;
    let running = until_status(&mut client, "wrapped", "running").await;
    assert!(running["mcp"][0]["pid"].is_u64(), "{running}");
    core.stop_extensions().await;
}
