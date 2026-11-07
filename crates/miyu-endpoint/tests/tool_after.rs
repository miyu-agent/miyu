//! 工具调用后的通知（施工 HK-2，设计 `65-工具挂接点.md` 第四节，`docs/reference/extensions.md` 的 `tool.after`）：真核心拉起
//! 测试扩展走一遍。
//!
//! - 结果落了盘以后收到一条，九格照第四节第 3 条：`args` 是模型写的原文，`blocks`、`by` 同日志里那一条；表里没有的工具不发；
//! - 照访问类别收的、被守卫拒了的也发，`by` 是拒的那个包；
//! - 扩展慢（不读）、崩了，回合照常，别的收件人照收；
//! - 没批 `events.read` 的、关掉的收不到；
//! - 场所会话照订事件流的规矩：桥只收自己平台的，别的扩展只收本机的；
//! - 收着通知时请求逐字节不变（第五节）。
//!
//! 脚本里调的在 `tool_after_more.rs`。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use miyu_session::testkit::Play;

use crate::support::extensions::*;
use crate::support::*;
use crate::tool_guard::{
    asked, install_guard, jot_once, lines, on_jot, results, second_request, started, talk,
    until_seen,
};

/// 装上收通知的扩展 `id`：清单声明 `events.read`、写上 `[after]`（`table`）、开着、批过；`args` 不写的照常记、握手、答。交回它
/// 记下的文件。
pub(crate) fn install_after(
    home: &Home,
    program: &Program,
    (id, table): (&str, &str),
    args: Option<Vec<String>>,
) -> PathBuf {
    let (path, step) = record(home, id);
    let args = args.unwrap_or_else(|| steps(&[&step, "hello", "serve"]));
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        &format!("opt/{id}/package.toml"),
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Audit\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"A\" }}\n\n[process]\nargs = [{args}]\nstart = \"always\"\ncapabilities = [\"events.read\"]\n\n[after]\n{table}\n",
            program = program.name(),
            args = args.join(", "),
        ),
    );
    approve(home, id, &["events.read"]);
    path
}

/// 记下的 `tool.after`，照先后。
pub(crate) fn told(path: &std::path::Path) -> Vec<Value> {
    asked(path, "tool.after")
}

#[tokio::test]
async fn a_notice_follows_the_written_result_with_its_fields() {
    let home = Home::new();
    let program = Program::new();
    let path = install_after(&home, &program, ("acme-audit", "tools = [\"jot\"]"), None);
    let plays = vec![
        Play::calls(&[("jot", r#"{"text":"hi"}"#), ("peek", "{}")]),
        Play::Says("好。"),
    ];
    let (core, jot, _) = started(&home, plays);
    lines(&path, 2).await;
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    until_seen(&path, |text| text.contains("tool.after")).await;
    assert_eq!(jot.calls().len(), 1);

    let results = results(&home, &session);
    let result = results
        .iter()
        .find(|result| result["body"]["call_id"] == told(&path)[0]["call_id"])
        .expect("通知的是落了盘的那一条");
    let notice = &told(&path)[0];
    assert_eq!(notice["session"], json!(session));
    assert_eq!(notice["tool"], json!("jot"));
    assert_eq!(notice["access"], json!("write"));
    assert_eq!(notice["status"], json!("ok"));
    assert_eq!(notice["status"], result["body"]["status"]);
    assert_eq!(notice["duration_ms"], result["body"]["duration_ms"]);
    assert!(notice["duration_ms"].is_u64(), "{notice}");
    assert_eq!(notice["by"], result["by"], "同日志里那一条的 by");
    assert_eq!(notice["blocks"], result["body"]["blocks"], "同日志里的原样");
    assert_eq!(
        notice["args"],
        json!(r#"{"text":"hi"}"#),
        "模型写的原文，同事件流里 tool_call 块的"
    );
    assert_eq!(
        notice.as_object().map(serde_json::Map::len),
        Some(9),
        "{notice}"
    );
    // 表里没有的 `peek` 不发：等一会儿也只有那一条。
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(told(&path).len(), 1, "{}", read(&path));
    core.stop_extensions().await;
}

#[tokio::test]
async fn access_picks_calls_and_a_blocked_call_is_told_who_blocked_it() {
    let home = Home::new();
    let programs = [Program::new(), Program::new()];
    install_guard(&home, &programs[0], &on_jot("acme-policy", "deny"));
    let path = install_after(
        &home,
        &programs[1],
        ("bravo-audit", "access = [\"write\"]"),
        None,
    );
    let (core, jot, _) = started(&home, jot_once());
    lines(&path, 2).await;
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    until_seen(&path, |text| text.contains("tool.after")).await;
    assert!(jot.calls().is_empty());
    let notice = &told(&path)[0];
    assert_eq!(notice["status"], json!("denied"));
    assert_eq!(notice["by"], json!({"kind": "module", "id": "acme-policy"}));
    assert!(notice.get("duration_ms").is_none(), "没真执行过：{notice}");
    assert_eq!(
        notice["blocks"][0]["text"],
        json!("Blocked by acme-policy: no writes here\n")
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_slow_or_crashed_listener_does_not_hold_the_turn() {
    let home = Home::new();
    let programs = [Program::new(), Program::new(), Program::new()];
    // 握了手就不再读（慢到底）、握了手就退出（崩了，照退避一再拉起）、照常读的。
    let (slow, slow_step) = record(&home, "acme-slow");
    install_after(
        &home,
        &programs[0],
        ("acme-slow", "tools = [\"jot\"]"),
        Some(steps(&[&slow_step, "hello", "hang"])),
    );
    let (crash, crash_step) = record(&home, "bravo-crash");
    install_after(
        &home,
        &programs[1],
        ("bravo-crash", "tools = [\"jot\"]"),
        Some(steps(&[&crash_step, "hello", "exit:3"])),
    );
    let fine = install_after(
        &home,
        &programs[2],
        ("charlie-audit", "tools = [\"jot\"]"),
        None,
    );
    // 一步四十次、每次的结果三十多 KB：慢的那个的管道、写队列早就满了。
    let text = "x".repeat(32 * 1024);
    let args = json!({ "text": text }).to_string();
    let calls: Vec<(&str, &str)> = (0..40).map(|_| ("jot", args.as_str())).collect();
    let (core, jot, _) = started(&home, vec![Play::calls(&calls), Play::Says("好。")]);
    lines(&slow, 2).await;
    lines(&crash, 2).await;
    lines(&fine, 2).await;
    let begun = Instant::now();
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    assert!(
        begun.elapsed() < Duration::from_secs(30),
        "回合不等收件人：{:?}",
        begun.elapsed()
    );
    assert_eq!(jot.calls().len(), 40);
    until_seen(&fine, |text| text.matches("\"tool.after\"").count() >= 40).await;
    assert_eq!(told(&fine).len(), 40, "别的收件人照收");
    let ids: Vec<Value> = told(&fine)
        .iter()
        .map(|notice| notice["call_id"].clone())
        .collect();
    let logged: Vec<Value> = results(&home, &session)
        .iter()
        .map(|result| result["body"]["call_id"].clone())
        .collect();
    assert_eq!(ids, logged, "照落盘的先后");
    core.stop_extensions().await;
}

#[tokio::test]
async fn listeners_without_events_read_or_switched_off_hear_nothing() {
    let home = Home::new();
    let programs = [Program::new(), Program::new(), Program::new()];
    let unapproved = install_after(
        &home,
        &programs[0],
        ("acme-audit", "tools = [\"jot\"]"),
        None,
    );
    let off = install_after(
        &home,
        &programs[1],
        ("bravo-audit", "tools = [\"jot\"]"),
        None,
    );
    let file = home.root.extensions_file();
    let switch = |change: &dyn Fn(&mut Value)| {
        let mut switches: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).expect("批过")).expect("JSON");
        change(&mut switches);
        std::fs::write(&file, switches.to_string()).expect("写得进");
    };
    // 批的不含 `events.read`：起不来，收不到。
    switch(&|switches| switches["approved"]["acme-audit"] = json!([]));
    let fine = install_after(
        &home,
        &programs[2],
        ("charlie-audit", "tools = [\"jot\"]"),
        None,
    );
    let (core, jot, _) = started(&home, jot_once());
    lines(&fine, 2).await;
    // 起来了、握了手，开关文件里再关掉（还没停下的那一阵）：发之前照开关看，不发。
    lines(&off, 2).await;
    switch(&|switches| switches["on"]["bravo-audit"] = json!(false));
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    until_seen(&fine, |text| text.contains("tool.after")).await;
    assert_eq!(jot.calls().len(), 1);
    assert!(told(&unapproved).is_empty(), "{}", read(&unapproved));
    assert!(told(&off).is_empty(), "{}", read(&off));
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_venue_session_is_told_only_to_its_bridge() {
    let home = Home::new();
    let programs = [Program::new(), Program::new()];
    let (bridge, step) = record(&home, "bot");
    let send =
        r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:10001"}}"#;
    let args = steps(&[
        &step,
        "hello",
        r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
        send,
        "listen",
    ]);
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        "opt/bot/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bridge\" }}\n\n[command]\nname = \"bot\"\nprogram = \"{}\"\nabout = {{ en = \"B\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\ncapabilities = {BRIDGE:?}\n\n[connection]\nplatform = \"qq\"\n\n[after]\ntools = [\"jot\"]\n",
            programs[0].name(),
            args.join(", ")
        ),
    );
    approve(&home, "bot", &BRIDGE);
    let audit = install_after(
        &home,
        &programs[1],
        ("acme-audit", "tools = [\"jot\"]"),
        None,
    );
    // 群里那一轮、本机那一轮各调一次 `jot`。
    let plays = vec![
        Play::calls(&[("jot", r#"{"text":"group"}"#)]),
        Play::Says("好。"),
        Play::calls(&[("jot", r#"{"text":"local"}"#)]),
        Play::Says("好。"),
    ];
    let core = crate::tool_after_more::served_with_jot(&home, plays);
    until_seen(&bridge, |text| text.contains("tool.after")).await;
    lines(&audit, 2).await;
    let (_client, local) = talk(&core).await;
    home.until_turns(&local, 1).await;
    until_seen(&audit, |text| text.contains("tool.after")).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let to_bridge = told(&bridge);
    assert_eq!(to_bridge.len(), 1, "{}", read(&bridge));
    assert_ne!(
        to_bridge[0]["session"],
        json!(local),
        "桥只收自己平台的场所会话"
    );
    assert_eq!(to_bridge[0]["args"], json!(r#"{"text":"group"}"#));
    let to_audit = told(&audit);
    assert_eq!(to_audit.len(), 1, "{}", read(&audit));
    assert_eq!(
        to_audit[0]["session"],
        json!(local),
        "别的扩展碰不到关起来的场所会话"
    );
    core.stop_extensions().await;
    core.stop_sessions().await;
}

#[tokio::test]
async fn listening_leaves_the_request_byte_for_byte_the_same() {
    let program = Program::new();
    let (without, _) = second_request(|_| None, "tool.after").await;
    let (with, told) = second_request(
        |home| {
            Some(install_after(
                home,
                &program,
                ("acme-audit", "tools = [\"jot\"]"),
                None,
            ))
        },
        "tool.after",
    )
    .await;
    assert_eq!(told, 1, "收到过");
    assert_eq!(with, without, "通知不进上下文，前缀零变化");
}
