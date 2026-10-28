//! MCP 服务的资源（施工 X-41，`docs/construction/X-41-MCP资源.md`）：真核心真起 `miyu-test-mcp`。声明了 `resources` 的服务跟着
//! 它的工具多两件 `list_resources`、`read_resource`，没声明的没有；列的一次一页、第一页带模板；读的字原样、图成图、别的二进制
//! 一行、没有的照服务的原话；只开放资源的服务说明照带；服务自己有同名工具的，服务的赢；预设关了这个服务的两件都没有；照缓存先
//! 登记的那一回也有。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

/// 核心：照 `script` 答，等法短，起好扩展。
fn started(home: &Home, script: &Script) -> Arc<Core> {
    let core = Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    core
}

/// 开好、等到在跑的几个包，交回连好的头。
async fn running(core: &Arc<Core>, packages: &[&str]) -> Client {
    let mut client = Client::connect(Arc::clone(core));
    client.hello().await;
    for package in packages {
        enable(&mut client, package).await;
        until_status(&mut client, package, "running").await;
    }
    client
}

/// 工具结果照工具名、字排：只读的几件一起跑，先后不定。
fn sorted(home: &Home, session: &str) -> Vec<(String, String, bool, usize)> {
    let mut got = results(home, session);
    got.sort();
    got
}

#[tokio::test]
async fn a_server_with_resources_gets_two_tools_that_list_and_read() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &["resources"]);
    bundle(&home, "plain", "plain", &[]);
    let script = Script::new([
        Play::calls(&[
            ("mcp__probe__list_resources", "{}"),
            ("mcp__probe__list_resources", r#"{"cursor":"2"}"#),
            (
                "mcp__probe__read_resource",
                r#"{"uri":"file:///notes.txt"}"#,
            ),
            ("mcp__probe__read_resource", r#"{"uri":"file:///dot.png"}"#),
            ("mcp__probe__read_resource", r#"{"uri":"file:///blob.bin"}"#),
            (
                "mcp__probe__read_resource",
                r#"{"uri":"file:///logs/monday.txt"}"#,
            ),
            ("mcp__probe__read_resource", r#"{"uri":"file:///nope"}"#),
        ]),
        Play::Says("好。"),
    ]);
    let core = started(&home, &script);
    let mut client = running(&core, &["probe", "plain"]).await;
    let session = open(&mut client).await;
    client.say("s1", &session, "看看资料").await;
    home.until_turns(&session, 1).await;
    let face = face(&script, 0);
    for name in ["mcp__probe__list_resources", "mcp__probe__read_resource"] {
        assert!(face.contains(&name.to_string()), "{name}：{face:?}");
    }
    for name in ["mcp__plain__list_resources", "mcp__plain__read_resource"] {
        assert!(
            !face.contains(&name.to_string()),
            "没声明资源的服务没有：{face:?}"
        );
    }
    let got = sorted(&home, &session);
    let pages: Vec<Value> = got
        .iter()
        .filter(|(name, ..)| name == "mcp__probe__list_resources")
        .map(|(_, text, error, _)| {
            assert!(!error, "{text}");
            serde_json::from_str(text).expect("一行 JSON")
        })
        .collect();
    let first = json!({
        "resources": [
            {"uri": "file:///notes.txt", "name": "notes", "mimeType": "text/plain"},
            {"uri": "file:///dot.png", "name": "dot", "mimeType": "image/png"},
        ],
        "resourceTemplates": [
            {"uriTemplate": "file:///logs/{day}.txt", "name": "day log", "mimeType": "text/plain"},
        ],
        "nextCursor": "2",
    });
    let second = json!({
        "resources": [
            {"uri": "file:///blob.bin", "name": "blob", "mimeType": "application/octet-stream"},
        ],
    });
    assert!(
        pages.contains(&first) && pages.contains(&second),
        "第一页带模板和下一页，第二页照 cursor、不带模板：{pages:?}"
    );
    let reads: Vec<(String, bool, usize)> = got
        .into_iter()
        .filter(|(name, ..)| name == "mcp__probe__read_resource")
        .map(|(_, text, error, images)| (text, error, images))
        .collect();
    let text = |text: &str| text.to_string();
    assert_eq!(
        reads,
        [
            (text(""), false, 1),
            (text("Log of monday."), false, 0),
            (text("Remember the milk."), false, 0),
            (text("Resource not found"), true, 0),
            (
                text("[application/octet-stream, 4 bytes, not shown]"),
                false,
                0
            ),
        ],
        "图成图、别的二进制一行、没有的照服务的原话"
    );
    core.stop_extensions().await;
}

/// 读资料是只读的（施工 X-41）：工作区这一级的会话里不问人就读。
#[tokio::test]
async fn reading_needs_no_asking_in_a_workspace_session() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &["resources"]);
    let script = Script::new([
        Play::calls(&[(
            "mcp__probe__read_resource",
            r#"{"uri":"file:///notes.txt"}"#,
        )]),
        Play::Says("好。"),
    ]);
    let core = started(&home, &script);
    let mut client = running(&core, &["probe"]).await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "读").await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let asked = loop {
        let log = home.log(&session);
        if log
            .iter()
            .any(|event| matches!(event.body, Body::ApprovalRequested(_)))
        {
            break true;
        }
        if log
            .iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_)))
        {
            break false;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到这一轮");
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert!(!asked, "只读的不问人");
    assert_eq!(
        results(&home, &session),
        [(
            "mcp__probe__read_resource".to_string(),
            "Remember the milk.".to_string(),
            false,
            0
        )]
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_server_with_only_resources_brings_its_instructions() {
    let home = Home::new();
    bundle(
        &home,
        "probe",
        "probe",
        &[
            "resources",
            "no-tools",
            "instructions:Read the notes first.",
        ],
    );
    let script = Script::new([Play::Says("好。")]);
    let core = started(&home, &script);
    let mut client = running(&core, &["probe"]).await;
    let session = open(&mut client).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    assert_eq!(
        face(&script, 0),
        ["mcp__probe__list_resources", "mcp__probe__read_resource"]
    );
    let system = script.requests()[0].1.system.clone();
    assert!(
        system.trim_end().ends_with(
            "<mcp-instructions server=\"probe\">\nRead the notes first.\n</mcp-instructions>"
        ),
        "只开放资源的服务也上了工具面，说明跟着带：{system}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn the_servers_own_tool_of_that_name_wins() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &["resources", "clash"]);
    let script = Script::new([
        Play::calls(&[(
            "mcp__probe__read_resource",
            r#"{"uri":"file:///notes.txt"}"#,
        )]),
        Play::Says("好。"),
    ]);
    let core = started(&home, &script);
    let mut client = running(&core, &["probe"]).await;
    let session = open(&mut client).await;
    client.say("s1", &session, "读").await;
    home.until_turns(&session, 1).await;
    assert!(face(&script, 0).contains(&"mcp__probe__list_resources".to_string()));
    assert_eq!(
        results(&home, &session),
        [(
            "mcp__probe__read_resource".to_string(),
            "the server's own".to_string(),
            false,
            0
        )]
    );
    let entry = entry(&mut client, "probe").await;
    assert!(
        entry["mcp"][0].get("skipped").is_none(),
        "让出去的那一件不算服务的工具没登记上：{entry}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_preset_turning_the_service_off_drops_both() {
    let home = Home::new();
    home.write(
        "presets/quiet.toml",
        "[preset]\nname = \"安静\"\n\n[features]\nmcp-probe = false\n",
    );
    bundle(&home, "probe", "probe", &["resources"]);
    let script = Script::new([Play::Says("好。")]);
    let core = started(&home, &script);
    let mut client = running(&core, &["probe"]).await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "preset": "quiet"}))
        .await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    assert_eq!(face(&script, 0), Vec::<String>::new());
    core.stop_extensions().await;
}

#[tokio::test]
async fn the_cache_registers_them_before_the_server_is_up() {
    let home = Home::new();
    let blocker = home.work.join("probe-down");
    let exit_if = format!("exit-if:{}", blocker.display());
    bundle(&home, "probe", "probe", &[&exit_if, "resources"]);
    let first = Script::new([]);
    let core = started(&home, &first);
    drop(running(&core, &["probe"]).await);
    let cache = home.root.state().join("mcp").join("probe.json");
    assert!(cache.is_file(), "列完写了缓存");
    core.stop_extensions().await;
    drop(core);
    std::fs::write(&blocker, "").expect("写得进");
    let script = Script::new([Play::Says("好。")]);
    let core = started(&home, &script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = open(&mut client).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let face = face(&script, 0);
    assert!(
        face.contains(&"mcp__probe__list_resources".to_string())
            && face.contains(&"mcp__probe__read_resource".to_string()),
        "服务起不来，照缓存先登记的也有这两件：{face:?}"
    );
    core.stop_extensions().await;
}
