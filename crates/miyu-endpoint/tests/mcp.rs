//! MCP 服务接进核心（施工 X-3 上，`docs/construction/X-3-MCP接进核心（上）.md`）：`bundle` 包写测试用的服务
//! （`miyu-test-mcp`），真核心真起进程走一遍。开了起、工具进目录、会话里调得到，几种结果、出错、超时各自交回；新旧两个时代都
//! 通；关了工具出目录、进程停；核心重启照开关起；起不来的退避着重试、停下；调用中途退出说暂时不可用、重启后又能调；工具变了
//! 换掉；包自己登记的工具不许占 `mcp__` 的名字。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

#[tokio::test]
async fn a_bundle_switched_on_serves_its_tools_in_both_eras() {
    let home = Home::new();
    let (path, record) = recording(&home, "probe");
    bundle(&home, "probe", "probe", &[&record]);
    bundle(&home, "old", "old", &["legacy"]);
    let script = Script::new([
        Play::calls(&[
            ("mcp__probe__echo", r#"{"text":"hi"}"#),
            ("mcp__probe__env", r#"{"text":"MIYU_MCP_PROBE"}"#),
            ("mcp__probe__cwd", "{}"),
            ("mcp__probe__picture", "{}"),
            ("mcp__probe__fail", "{}"),
            ("mcp__probe__error", "{}"),
            ("mcp__probe__garbled", "{}"),
            ("mcp__probe__slow", "{}"),
            ("mcp__old__echo", r#"{"text":"old"}"#),
        ]),
        Play::Says("好。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let off = entry(&mut client, "probe").await;
    assert_eq!(
        (off["status"].clone(), off["enabled"].clone()),
        (json!("off"), json!(false)),
        "bundle 没写开关的关着：{off}"
    );
    let enabled = enable(&mut client, "probe").await;
    assert_eq!(enabled["result"]["enabled"], true, "{enabled}");
    let enabled = enable(&mut client, "old").await;
    assert_eq!(enabled["result"]["enabled"], true, "{enabled}");
    until_status(&mut client, "probe", "running").await;
    until_status(&mut client, "old", "running").await;

    let session = open(&mut client).await;
    client.say("s1", &session, "调一下").await;
    home.until_turns(&session, 1).await;
    let face = face(&script, 0);
    assert!(face.contains(&"mcp__probe__echo".to_string()), "{face:?}");
    assert!(face.contains(&"mcp__old__echo".to_string()), "{face:?}");
    // 工作目录照真实路径比（macOS 的 /var 是链接、Windows 的规范路径带前缀）：只看最后两段。
    let mut got = results(&home, &session);
    let cwd = got
        .iter_mut()
        .find(|(name, ..)| name == "mcp__probe__cwd")
        .expect("调了 cwd");
    assert!(Path::new(&cwd.1).ends_with("mcp/probe"), "{cwd:?}");
    cwd.1 = "<cwd>".to_string();
    let text = |text: &str| text.to_string();
    assert_eq!(
        got,
        [
            ("mcp__old__echo".to_string(), text("old"), false, 0),
            ("mcp__probe__cwd".to_string(), text("<cwd>"), false, 0),
            ("mcp__probe__echo".to_string(), text("hi"), false, 0),
            ("mcp__probe__env".to_string(), text("set"), false, 0),
            (
                "mcp__probe__error".to_string(),
                text("server says no"),
                true,
                0
            ),
            ("mcp__probe__fail".to_string(), text("broke"), true, 0),
            (
                "mcp__probe__garbled".to_string(),
                text(
                    "The tool \"mcp__probe__garbled\" stopped because of an internal error. It may have been partly done.\n"
                ),
                true,
                0
            ),
            (
                "mcp__probe__picture".to_string(),
                text("a picture"),
                false,
                1
            ),
            (
                "mcp__probe__slow".to_string(),
                text(
                    "The tool \"mcp__probe__slow\" did not answer within 1 seconds. It may have been partly done.\n"
                ),
                true,
                0
            ),
        ]
    );
    // 超时的那一次发了取消。
    until_line(&path, "notifications/cancelled").await;
    core.stop_extensions().await;
}

#[tokio::test]
async fn switching_off_withdraws_the_tools_and_a_restart_follows_the_switch() {
    let home = Home::new();
    let (path, record) = recording(&home, "probe");
    bundle(&home, "probe", "probe", &[&record]);
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("在。")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    let session = open(&mut client).await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    assert!(!face(&script, 0).is_empty(), "开着的有工具");

    let disabled = client
        .call("d", "package.disable", json!({"package": "probe"}))
        .await;
    assert_eq!(
        (
            disabled["result"]["status"].clone(),
            disabled["result"]["enabled"].clone()
        ),
        (json!("off"), json!(false)),
        "{disabled}"
    );
    // 停的时候关了它的标准输入，它读到头自己退出。
    until_line(&path, "eof").await;
    client.say("s2", &session, "还在吗").await;
    home.until_turns(&session, 2).await;
    assert!(
        face(&script, 1).is_empty(),
        "关了的下一个回合没有：{:?}",
        face(&script, 1)
    );
    let started = read(&path).matches("tools/list").count();
    core.stop_extensions().await;
    drop(client);

    // 核心重启：关着的不起；开着的照开关起。
    let again = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    again.start_extensions();
    // 关着的不起：起了的话这一刻就在「起着」，核心不空闲（还没有连接、没有回合）。
    assert!(again.idle().await, "关着的服务没起");
    let mut client = Client::connect(Arc::clone(&again));
    client.hello().await;
    assert_eq!(entry(&mut client, "probe").await["status"], "off");
    assert_eq!(
        read(&path).matches("tools/list").count(),
        started,
        "关着的没起"
    );
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    again.stop_extensions().await;
    drop(client);
    let third = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    third.start_extensions();
    let mut client = Client::connect(Arc::clone(&third));
    client.hello().await;
    until_status(&mut client, "probe", "running").await;
    third.stop_extensions().await;
}

#[tokio::test]
async fn a_server_that_keeps_exiting_is_stopped_with_its_stderr_kept() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &["stray:cannot start", "exit:3"]);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "stopped").await;
    let stderr = read(&home.root.state().join("logs").join("mcp-probe.stderr"));
    assert!(stderr.contains("cannot start"), "{stderr}");
    assert_eq!(
        stderr.matches("cannot start").count(),
        5,
        "试了 5 次：{stderr}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_server_that_dies_mid_call_is_unavailable_then_comes_back() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &[]);
    let script = Script::new([
        Play::calls(&[("mcp__probe__die", "{}")]),
        Play::Says("挂了。"),
        Play::calls(&[("mcp__probe__echo", r#"{"text":"back"}"#)]),
        Play::Says("回来了。"),
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
    let session = open(&mut client).await;
    client.say("s1", &session, "挂掉").await;
    home.until_turns(&session, 1).await;
    // 退出以后退避着重启，又在跑了。
    until_status(&mut client, "probe", "running").await;
    client.say("s2", &session, "再来").await;
    home.until_turns(&session, 2).await;
    assert_eq!(
        results(&home, &session),
        [
            (
                "mcp__probe__die".to_string(),
                "The tool \"mcp__probe__die\" is not available right now.\n".to_string(),
                true,
                0
            ),
            ("mcp__probe__echo".to_string(), "back".to_string(), false, 0),
        ]
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn changed_tools_are_listed_again() {
    let home = Home::new();
    let (path, record) = recording(&home, "probe");
    bundle(&home, "probe", "probe", &[&record, "changed"]);
    let script = Script::new([Play::Says("好。")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    // 第二次列完、登记了才算：等到第二次列的回应发出去以后，核心那边登记是同步的，再等一小会儿。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).matches("tools/list").count() < 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "没再列：{}",
            read(&path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let session = open(&mut client).await;
    client.say("s1", &session, "有什么").await;
    home.until_turns(&session, 1).await;
    assert!(
        face(&script, 0).contains(&"mcp__probe__later".to_string()),
        "{:?}",
        face(&script, 0)
    );
    core.stop_extensions().await;
}

/// 包自己登记的工具不许占 `mcp__` 开头的名字（施工 X-3 上）：留给 MCP 服务，不然预设会把它归到服务的功能上。
#[tokio::test]
async fn a_provider_cannot_take_an_mcp_name() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    let tools = json!({"tools": [{"name": "mcp__x__y", "description": "Y.",
        "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]});
    install_providing(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[&step, "hello", &format!("ask:provide:{tools}"), "wait"]),
    );
    let core = core(&home, quick());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 3 {
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let answer: Value = serde_json::from_str(read(&path).lines().nth(2).unwrap()).unwrap();
    assert_eq!(answer["error"]["data"]["reason"], "bad_tool", "{answer}");
    assert_eq!(answer["error"]["data"]["problem"], "name");
    core.stop_extensions().await;
}
