//! 个人设置里的 MCP 服务（施工 F-9，`docs/blueprint/mcp.md`「个人设置里的一张表」）：`miyu check` 照行报写错的、报盖住了包里
//! 同名服务的；`[mcp.<名字>]` 不算配置里不认识的键（F-9 上）。核心照它真起测试用的服务（`miyu-test-mcp`）：写了就起、挂在
//! `~home` 下、预设里一个功能，盖住包里同名的（那个不起）；手改个人设置当场换：新写的起、包里同名的停下，删掉的停下、包里的
//! 照开关起回来（F-9 下）。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::{Program, install, steps, until_state};
use crate::support::mcp::*;
use crate::support::venues::configured_core;
use crate::support::*;

/// 带一个叫 `github` 的服务的包。
const KIT: &str = "[package]\nname = { en = \"Kit\" }\n\n[[mcp]]\nname = \"github\"\ncommand = [\"kit-github\"]\n";

/// 个人设置：界面语言，一个盖住包里那个的服务，一个写错的。
const SETTINGS: &str = "[ui]\nlanguage = \"zh\"\n\n[mcp.github]\ncommand = [\"npx\", \"server-github\"]\n\n[mcp.empty]\ncommand = []\n";

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(configured_core(home, &Script::new([]), Catalog::default()));
    client.hello().await;
    client
}

/// 一处的几格：种类、文件、代码、级别、第几行。
fn brief(problem: &Value) -> (String, String, String, String, Option<u64>) {
    let text = |key: &str| problem[key].as_str().unwrap_or_default().to_string();
    (
        text("kind"),
        text("file"),
        text("code"),
        text("level"),
        problem["line"].as_u64(),
    )
}

fn row(
    file: &str,
    code: &str,
    level: &str,
    line: u64,
) -> (String, String, String, String, Option<u64>) {
    (
        "mcp".to_string(),
        file.to_string(),
        code.to_string(),
        level.to_string(),
        Some(line),
    )
}

#[tokio::test]
async fn check_reports_wrong_servers_and_covered_package_servers() {
    let home = Home::new();
    home.write("packages/kit/package.toml", KIT);
    home.write("config.toml", SETTINGS);
    let mut client = connected(&home).await;
    let reply = client.call("k1", "check", json!({})).await;
    let problems = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .clone();
    let found: Vec<_> = problems.iter().map(brief).collect();
    assert_eq!(
        found,
        [
            row("config.toml", "bad_mcp", "error", 7),
            row("packages/kit/package.toml", "mcp_covered", "warning", 4),
        ],
        "[mcp.<名字>] 不算不认识的键：{reply}"
    );
    let covered = problems[1]["message"].as_str().unwrap_or_default();
    assert!(
        covered.contains("config.toml [mcp.github]"),
        "说被谁盖住：{covered}"
    );
    // 只查个人设置这一份：照样报，盖住了的也报。
    let path = home.root.path().join("config.toml");
    let one = client
        .call("k2", "check", json!({"file": path.to_string_lossy()}))
        .await;
    let found: Vec<_> = one["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{one}"))
        .iter()
        .map(brief)
        .collect();
    assert_eq!(found.len(), 2, "{one}");
}

#[tokio::test]
async fn the_mcp_table_does_not_disturb_the_rest_of_the_settings() {
    let home = Home::new();
    home.write("config.toml", SETTINGS);
    let mut client = connected(&home).await;
    let reply = client
        .call("k1", "config.get", json!({"keys": ["ui.language"]}))
        .await;
    assert_eq!(
        reply["result"]["items"]["ui.language"]["value"], "zh",
        "{reply}"
    );
    assert_eq!(reply["result"]["problems"], json!([]), "{reply}");
}

/// 说一句、等第 `n` 轮说完，交回这一轮最后一个工具结果的字（剧本里调的是 `mcp__probe__env`）。
async fn env_answer(home: &Home, client: &mut Client, session: &str, n: usize) -> String {
    client.say(&format!("s{n}"), session, "看一下").await;
    home.until_turns(session, n).await;
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .next_back()
        .unwrap_or_default()
}

/// 等到文件 `path` 在：最多 60 秒。
async fn until_exists(path: &Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !path.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} 没出现",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_server_in_the_settings_runs_and_covers_the_package_one() {
    let home = Home::new();
    let (theirs, record) = recording(&home, "package");
    bundle(&home, "kit", "probe", &[&record]);
    let (ours, record) = recording(&home, "settings");
    home.write("config.toml", &mine(&record));
    let script = Script::new([
        Play::calls(&[("mcp__probe__env", r#"{"text":"MIYU_MCP_PROBE"}"#)]),
        Play::Says("好。"),
    ]);
    let core = configured(&home, &script);
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "kit").await;
    until_exists(&ours).await;
    let session = open(&mut client).await;
    assert_eq!(
        env_answer(&home, &mut client, &session, 1).await,
        "mine",
        "调到的是个人设置里的那个"
    );
    assert!(!theirs.exists(), "包里同名的不起");
    let kit = entry(&mut client, "kit").await;
    assert_eq!(
        kit["status"], "off",
        "服务都被盖住的包照关着算，不卡在起着：{kit}"
    );
    let listed = client
        .call("g", "preset.get", json!({"preset": "full"}))
        .await;
    let probes: Vec<&Value> = listed["result"]["features"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .filter(|one| one["id"] == "mcp-probe")
        .collect();
    assert_eq!(probes.len(), 1, "一个名字一个功能：{listed}");
    assert_eq!(probes[0]["name"], "probe");
    let tools = probes[0]["tools"].as_array().expect("列出工具");
    assert!(
        tools.iter().any(|tool| tool["name"] == "mcp__probe__env"),
        "{listed}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn editing_the_settings_starts_and_stops_servers_on_the_spot() {
    let home = Home::new();
    home.root.prepare_workspace().expect("建得了家目录");
    let (theirs, record) = recording(&home, "package");
    bundle(&home, "kit", "probe", &[&record]);
    home.write("config.toml", "[ui]\nlanguage = \"zh\"\n");
    let calls = || Play::calls(&[("mcp__probe__env", r#"{"text":"MIYU_MCP_PROBE"}"#)]);
    let script = Script::new([
        calls(),
        Play::Says("好。"),
        calls(),
        Play::Says("好。"),
        calls(),
        Play::Says("好。"),
        calls(),
        Play::Says("好。"),
    ]);
    let core = configured(&home, &script);
    let _watching = core.watch_config().expect("监视得了");
    core.start_extensions();
    let _following = core.follow_home_mcp();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "kit").await;
    until_status(&mut client, "kit", "running").await;
    let session = open(&mut client).await;
    assert_eq!(
        env_answer(&home, &mut client, &session, 1).await,
        "set",
        "先是包里的"
    );

    // 写进个人设置：包里同名的停下，个人设置里的起来。
    let (ours, record) = recording(&home, "settings");
    home.write(
        "config.toml",
        &format!("[ui]\nlanguage = \"zh\"\n\n{}", mine(&record)),
    );
    until_exists(&ours).await;
    until_line(&ours, "tools/list").await;
    assert_eq!(env_answer(&home, &mut client, &session, 2).await, "mine");

    // 改了：停下、照新写的再起。
    let (changed, record) = recording(&home, "changed");
    home.write(
        "config.toml",
        &format!(
            "[ui]\nlanguage = \"zh\"\n\n{}",
            mine(&record).replace("\"mine\"", "\"again\"")
        ),
    );
    until_exists(&changed).await;
    until_line(&changed, "tools/list").await;
    assert_eq!(env_answer(&home, &mut client, &session, 3).await, "again");

    // 删掉：个人设置里的停下，包里的照开关起回来。
    std::fs::remove_file(&theirs).expect("删得掉");
    home.write("config.toml", "[ui]\nlanguage = \"zh\"\n");
    until_exists(&theirs).await;
    until_line(&theirs, "tools/list").await;
    assert_eq!(env_answer(&home, &mut client, &session, 4).await, "set");
    core.stop_extensions().await;
}

/// 扩展包 `relay`（照 `start` 起）带一个服务 `xsvc`，记到 `record`。
fn relay(home: &Home, program: &Program, start: &str, record: &str) {
    install(
        home,
        "relay",
        &program.name(),
        start,
        &steps(&["hello", "wait"]),
    );
    let manifest = home.root.path().join("packages/relay/package.toml");
    let mut text = std::fs::read_to_string(&manifest).expect("读得了清单");
    text.push_str(&format!(
        "\n[[mcp]]\nname = \"xsvc\"\ncommand = ['{SERVER}', '{record}']\n"
    ));
    std::fs::write(&manifest, text).expect("写得进");
}

/// 个人设置里一张 `[mcp.xsvc]`，记到 `record`。
fn ours_xsvc(record: &str) -> String {
    format!("[mcp.xsvc]\ncommand = ['{SERVER}', '{record}']\n")
}

/// 起来、开始监视配置和跟着个人设置换的核心，连上的连接。
async fn following(
    home: &Home,
) -> (
    Arc<Core>,
    miyu_store::watch::Watch,
    tokio::task::JoinHandle<()>,
    Client,
) {
    home.root.prepare_workspace().expect("建得了家目录");
    let core = configured(home, &Script::new([]));
    let watching = core.watch_config().expect("监视得了");
    core.start_extensions();
    let follow = core.follow_home_mcp();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    (core, watching, follow, client)
}

#[tokio::test]
async fn a_running_extension_gets_its_service_back_when_the_settings_let_go() {
    let home = Home::new();
    let program = Program::new();
    let (theirs, record) = recording(&home, "relay");
    relay(&home, &program, "always", &record);
    let (ours, mine) = recording(&home, "settings");
    home.write("config.toml", &ours_xsvc(&mine));
    let (core, _watching, _follow, mut client) = following(&home).await;
    until_state(&mut client, "relay", |one| one["state"] == "running").await;
    until_line(&ours, "tools/list").await;
    assert!(!theirs.exists(), "扩展在跑，它带的同名服务照样被盖住");
    home.write("config.toml", "[ui]\nlanguage = \"zh\"\n");
    until_line(&ours, "eof").await;
    until_line(&theirs, "tools/list").await;
    core.stop_extensions().await;
}

#[tokio::test]
async fn an_extension_switched_off_stays_off_when_the_settings_let_go() {
    let home = Home::new();
    let program = Program::new();
    let (theirs, record) = recording(&home, "relay");
    relay(&home, &program, "manual", &record);
    let (ours, mine) = recording(&home, "settings");
    home.write("config.toml", &ours_xsvc(&mine));
    let (core, _watching, _follow, _client) = following(&home).await;
    until_line(&ours, "tools/list").await;
    home.write("config.toml", "[ui]\nlanguage = \"zh\"\n");
    until_line(&ours, "eof").await;
    // 放开以后不该起：等一会儿，扩展没开，它带的服务照样不在。
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!theirs.exists(), "扩展没开，它带的服务不起");
    core.stop_extensions().await;
}
