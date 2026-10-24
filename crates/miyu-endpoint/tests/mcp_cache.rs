//! MCP 服务接进核心的下半（施工 X-3 下，`docs/construction/X-3-MCP接进核心（下）.md`）：真核心真起 `miyu-test-mcp` 走一遍。
//! 目录缓存：核心重启以后服务起不来，照缓存登记的工具照样在工具面上、调到说暂时不可用；缓存记着旧时代的直接握手、不再探。
//! `ttlMs` 过了、有人用它时再列一次。扩展包带的服务跟着扩展起停。卸包清掉服务的缓存和工作目录，工具记成随包卸掉。

use std::sync::Arc;

use serde_json::json;

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

/// 核心：照 `script` 答，等法短。
fn started(home: &Home, script: &Script) -> Arc<miyu_endpoint::Core> {
    let core = Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    core
}

#[tokio::test]
async fn a_cached_catalog_stands_in_until_the_server_is_up() {
    let home = Home::new();
    let (path, record) = recording(&home, "probe");
    let flag = home.work.join("probe-down");
    let exit_if = format!("exit-if:{}", flag.display());
    bundle(&home, "probe", "probe", &["legacy", &exit_if, &record]);
    let script = Script::new([
        Play::calls(&[("mcp__probe__echo", r#"{"text":"hi"}"#)]),
        Play::Says("不在。"),
    ]);
    let core = started(&home, &script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    let cache = home.root.state().join("mcp").join("probe.json");
    assert!(cache.is_file(), "列完写了缓存");
    core.stop_extensions().await;
    drop(client);

    // 服务起不来了：照缓存登记的工具照样在工具面上，调到说暂时不可用。
    std::fs::write(&flag, b"").unwrap();
    let down = started(&home, &script);
    let mut client = Client::connect(Arc::clone(&down));
    client.hello().await;
    let session = open(&mut client).await;
    client.say("s1", &session, "调一下").await;
    home.until_turns(&session, 1).await;
    assert!(
        face(&script, 0).contains(&"mcp__probe__echo".to_string()),
        "{:?}",
        face(&script, 0)
    );
    assert_eq!(
        results(&home, &session),
        [(
            "mcp__probe__echo".to_string(),
            "The tool \"mcp__probe__echo\" is not available right now.\n".to_string(),
            true,
            0
        )]
    );
    down.stop_extensions().await;
    drop(client);

    // 又起得来了：缓存记着是旧时代，直接握手、不再探。
    std::fs::remove_file(&flag).unwrap();
    let up = started(&home, &Script::new([]));
    let mut client = Client::connect(Arc::clone(&up));
    client.hello().await;
    until_status(&mut client, "probe", "running").await;
    let lines = read(&path);
    assert_eq!(lines.matches("server/discover").count(), 1, "{lines}");
    assert_eq!(lines.matches("\"initialize\"").count(), 2, "{lines}");
    up.stop_extensions().await;
}

#[tokio::test]
async fn an_expired_catalog_is_listed_again_when_used() {
    let home = Home::new();
    let (path, record) = recording(&home, "probe");
    bundle(&home, "probe", "probe", &["ttl:1", &record]);
    let script = Script::new([
        Play::calls(&[("mcp__probe__echo", r#"{"text":"hi"}"#)]),
        Play::Says("好。"),
        Play::Says("嗯。"),
    ]);
    let core = started(&home, &script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    assert_eq!(read(&path).matches("tools/list").count(), 1, "没人用不再列");
    let session = open(&mut client).await;
    client.say("s1", &session, "调一下").await;
    home.until_turns(&session, 1).await;
    assert!(!face(&script, 0).contains(&"mcp__probe__later".to_string()));
    // 用了一次：过期了，再列一次，下一个回合换上。
    until_count(&path, "tools/list", 2).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    client.say("s2", &session, "再看看").await;
    home.until_turns(&session, 2).await;
    assert!(
        face(&script, 2).contains(&"mcp__probe__later".to_string()),
        "{:?}",
        face(&script, 2)
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn an_extension_carries_its_services_on_and_off() {
    let home = Home::new();
    let program = Program::new();
    let (path, record) = recording(&home, "xsvc");
    install(
        &home,
        "relay",
        &program.name(),
        "always",
        &steps(&["hello", "wait"]),
    );
    let manifest = home.root.path().join("system/packages/relay/package.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text.push_str(&format!(
        "\n[[mcp]]\nname = \"xsvc\"\ncommand = ['{SERVER}', '{record}']\nneed_load = false\n"
    ));
    std::fs::write(&manifest, text).unwrap();
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let core = started(&home, &script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    until_state(&mut client, "relay", |one| one["state"] == "running").await;
    until_count(&path, "tools/list", 1).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let session = open(&mut client).await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    assert!(
        face(&script, 0).contains(&"mcp__xsvc__echo".to_string()),
        "扩展开着，它带的服务也开着：{:?}",
        face(&script, 0)
    );

    let off = call(&mut client, "extension.disable", "relay").await;
    assert_eq!(off["result"]["on"], false, "{off}");
    until_line(&path, "eof").await;
    client.say("s2", &session, "还在吗").await;
    home.until_turns(&session, 2).await;
    assert!(face(&script, 1).is_empty(), "{:?}", face(&script, 1));
    core.stop_extensions().await;
}

#[tokio::test]
async fn removing_a_bundle_clears_its_cache_and_working_dir() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &[]);
    let script = Script::new([
        Play::Says("好。"),
        Play::calls(&[("mcp__probe__echo", "{}")]),
        Play::Says("没了。"),
    ]);
    let core = started(&home, &script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    let session = open(&mut client).await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let cache = home.root.state().join("mcp").join("probe.json");
    let dir = home.root.state().join("mcp").join("probe");
    assert!(cache.is_file() && dir.is_dir());

    let removed = client
        .call("r", "package.remove", json!({"package": "probe"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert!(!cache.exists() && !dir.exists(), "缓存、工作目录清掉了");
    client.say("s2", &session, "再调").await;
    home.until_turns(&session, 2).await;
    assert_eq!(
        results(&home, &session),
        [(
            "mcp__probe__echo".to_string(),
            "The tool \"mcp__probe__echo\" was uninstalled.\n".to_string(),
            true,
            0
        )],
        "用过它的会话调到说已卸载"
    );
    core.stop_extensions().await;
}

/// 等到 `path` 里 `needle` 出现 `count` 次：最多 60 秒。
async fn until_count(path: &std::path::Path, needle: &str, count: usize) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    while read(path).matches(needle).count() < count {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {count} 次 {needle}：{}",
            read(path)
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
