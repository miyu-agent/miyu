//! 工具调用前的守卫的另几种（施工 HK-1，设计 65 第三节第 1、2、5、6、7 条；`tool_guard.rs` 是主路）：
//!
//! - 超时、回错误、崩了、开着却没在跑的，都算拒绝，理由是核心自己的那几句；超时的另收到 `guard.cancel`；
//! - 打断时还在等的照打断收尾，不等它答，它收到 `guard.cancel`；
//! - 主人关掉的不问；它自己登记的工具不问它；权限策略先拒了的不问；
//! - 脚本里调的（`miyu call`）同样过守卫。

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use miyu_kernel::tool::Access;
use miyu_session::testkit::Play;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::extensions::*;
use crate::support::*;
use crate::tool_guard::{
    Guard, asked, install_guard, install_guard_with, jot_once, lines, on_jot, results, started,
    talk, text_of, until_seen,
};

/// 守卫 `id`：照 `how` 答，`[guard]` 那张表是 `table`。
fn waiting(id: &'static str, how: &'static str, table: &'static str) -> Guard {
    Guard { id, how, table }
}

/// 说一轮 `jot`，交回那一次的结果、`jot` 跑了几次；守卫记下的字照 `seen` 等到了才停扩展、收拾数据根。
async fn denied_once(guard: &Guard, seen: impl Fn(&str) -> bool) -> (Value, usize, String) {
    let home = Home::new();
    let program = Program::new();
    let path = install_guard(&home, &program, guard);
    let (core, jot, _) = started(&home, jot_once());
    lines(&path, 2).await;
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    let ran = jot.calls().len();
    let result = results(&home, &session)[0].clone();
    until_seen(&path, seen).await;
    core.stop_extensions().await;
    (result, ran, read(&path))
}

#[tokio::test]
async fn a_silent_guard_times_out_into_a_denial_and_is_told_to_cancel() {
    let guard = waiting(
        "acme-policy",
        "silent",
        "tools = [\"jot\"]\ntimeout_ms = 200",
    );
    let started_at = Instant::now();
    let (result, ran, recorded) = denied_once(&guard, |text| text.contains("guard.cancel")).await;
    assert_eq!(ran, 0);
    assert_eq!(result["body"]["status"], json!("denied"));
    assert_eq!(
        text_of(&result),
        "Blocked by acme-policy: the guard did not answer in time\n"
    );
    assert_eq!(
        result["body"]["human"]["key"],
        json!("core/guard/timed-out")
    );
    assert!(
        started_at.elapsed() < Duration::from_secs(30),
        "照清单的时限，不卡住"
    );
    let cancelled = called(&recorded, "guard.cancel");
    assert_eq!(cancelled[0]["call_id"], result["body"]["call_id"]);
    assert!(cancelled[0]["session"].is_string());
}

/// 记下的字里核心发来的 `method` 的参数，照先后。
fn called(recorded: &str, method: &str) -> Vec<Value> {
    recorded
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| line["method"] == json!(method))
        .map(|line| line["params"].clone())
        .collect()
}

#[tokio::test]
async fn a_guard_that_errors_or_crashes_blocks_the_call() {
    let (result, ran, _) = denied_once(&on_jot("acme-policy", "boom"), |_| true).await;
    assert_eq!(ran, 0);
    assert_eq!(
        text_of(&result),
        "Blocked by acme-policy: the guard returned an error\n"
    );
    // 崩了：问到它的时候它退出了，连接断了。
    let (result, ran, _) = denied_once(&on_jot("acme-policy", "crash"), |_| true).await;
    assert_eq!(ran, 0);
    assert_eq!(
        text_of(&result),
        "Blocked by acme-policy: the guard is not running\n"
    );
    assert_eq!(result["by"], json!({"kind": "module", "id": "acme-policy"}));
}

#[tokio::test]
async fn a_guard_switched_on_but_not_running_blocks_without_being_asked() {
    // 起来了却一直不握手（正在起）：不问它，照拒。
    let home_steps =
        |path: &std::path::Path| steps(&[&format!("record:{}", path.display()), "hang"]);
    let home = Home::new();
    let program = Program::new();
    let guard = on_jot("acme-policy", "allow");
    let (path, _) = record(&home, guard.id);
    let path = install_guard_with(&home, &program, &guard, Some(home_steps(&path)));
    let (core, jot, _) = started(&home, jot_once());
    lines(&path, 1).await;
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    assert!(jot.calls().is_empty(), "失效往严里走");
    let result = &results(&home, &session)[0];
    assert_eq!(
        text_of(result),
        "Blocked by acme-policy: the guard is not running\n"
    );
    assert!(asked(&path, "tool.guard").is_empty());
    core.stop_extensions().await;
}

#[tokio::test]
async fn an_interrupt_ends_the_wait_and_tells_the_guard_to_cancel() {
    let home = Home::new();
    let program = Program::new();
    let guard = waiting(
        "acme-policy",
        "silent",
        "tools = [\"jot\"]\ntimeout_ms = 5000",
    );
    let path = install_guard(&home, &program, &guard);
    let (core, jot, _) = started(&home, jot_once());
    lines(&path, 2).await;
    let (mut client, session) = talk(&core).await;
    until_seen(&path, |text| text.contains("tool.guard")).await;
    let begun = Instant::now();
    let reply = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "keep"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    assert!(
        begun.elapsed() < Duration::from_secs(4),
        "不等守卫答（时限 5 秒）"
    );
    assert!(jot.calls().is_empty());
    let result = &results(&home, &session)[0];
    assert_eq!(result["body"]["status"], json!("cancelled"), "{result}");
    until_seen(&path, |text| text.contains("guard.cancel")).await;
    assert!(
        begun.elapsed() < Duration::from_secs(4),
        "打断当场发，不是等到超时才发"
    );
    let asked = asked(&path, "tool.guard");
    let cancelled = crate::tool_guard::asked(&path, "guard.cancel");
    assert_eq!(cancelled.len(), 1);
    assert_eq!(cancelled[0]["call_id"], asked[0]["call_id"]);
    assert_eq!(cancelled[0]["session"], json!(session));
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_switched_off_guard_is_not_asked() {
    let home = Home::new();
    let program = Program::new();
    let path = install_guard(&home, &program, &on_jot("acme-policy", "deny"));
    // 主人关掉：开关文件里写 `false`。
    let file = home.root.extensions_file();
    let mut switches: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("批过")).expect("JSON");
    switches["on"]["acme-policy"] = json!(false);
    std::fs::write(&file, switches.to_string()).expect("写得进");
    let (core, jot, _) = started(&home, jot_once());
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    assert_eq!(jot.calls().len(), 1, "关了就是不要这个守卫");
    assert!(read(&path).is_empty(), "没拉起、没问");
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_guard_is_not_asked_about_its_own_tools_or_what_the_policy_refused() {
    let home = Home::new();
    let program = Program::new();
    // 它自己登记 `own_jot`，也守着 `own_jot` 和 `jot`：`jot` 写数据根，权限策略先拒了。
    let tools = json!({"tools": [{"name": "own_jot", "description": "A test tool.",
        "input_schema": {"type": "object"}, "access": "write", "venues": ["local"]}]});
    let guard = waiting("acme-policy", "deny", "tools = [\"own_jot\", \"jot\"]");
    let (path, step) = record(&home, guard.id);
    let args = steps(&[
        &step,
        "hello",
        &format!("ask:provide:{tools}"),
        "guard:deny",
        "serve",
    ]);
    install_guard_with(&home, &program, &guard, Some(args));
    approve(&home, guard.id, &["tools"]);
    rewrite_capabilities(&home, guard.id);
    let inside = json!({"path": home.root.path().join("secret.txt")}).to_string();
    let plays = vec![
        Play::calls(&[("own_jot", "{}")]),
        Play::calls(&[("jot", inside.as_str())]),
        Play::Says("好。"),
    ];
    let (core, jot, _) = started(&home, plays);
    lines(&path, 3).await;
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    let results = results(&home, &session);
    assert_eq!(
        results[0]["body"]["status"],
        json!("ok"),
        "自己的工具不问自己：{}",
        results[0]
    );
    assert_eq!(results[1]["body"]["status"], json!("denied"));
    assert_eq!(
        results[1]["by"],
        json!({"kind": "module", "id": "permissions"}),
        "策略拒的"
    );
    assert!(jot.calls().is_empty());
    assert!(
        asked(&path, "tool.guard").is_empty(),
        "一次都没问：{}",
        read(&path)
    );
    core.stop_extensions().await;
}

/// 包 `id` 的清单声明 `tools`、`tool.guard` 两项（[`install_guard_with`] 只写后一项）。
fn rewrite_capabilities(home: &Home, id: &str) {
    let path = home.root.path().join(format!("opt/{id}/package.toml"));
    let text = std::fs::read_to_string(&path).expect("装了");
    let text = text.replace(
        "capabilities = [\"tool.guard\"]",
        "capabilities = [\"tools\", \"tool.guard\"]",
    );
    std::fs::write(&path, text).expect("写得进");
}

/// 假的 `shell`：命令里经脚本的端口调一次 `peek`；守卫装成 `guard`。起核心，等守卫握了手，交回核心、`peek`。
async fn scripted(
    home: &Home,
    program: &Program,
    guard: &Guard,
) -> (Arc<miyu_endpoint::Core>, Arc<Fake>, std::path::PathBuf) {
    let path = install_guard(home, program, guard);
    let shell = Fake::commanding(
        "shell",
        Access::Read,
        Some(miyu_tool::RUN_IN_BACKGROUND),
        Act::Calls(vec![("peek", "{}")]),
    );
    let peek = Fake::callable("peek", Access::Read, Act::Echo);
    let catalog = Catalog::new([
        Arc::clone(&shell) as Arc<dyn Tool>,
        Arc::clone(&peek) as Arc<dyn Tool>,
    ])
    .expect("合写法");
    let script = miyu_session::testkit::Script::new([
        Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, catalog, None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 2).await;
    (core, peek, path)
}

#[tokio::test]
async fn tools_called_from_scripts_pass_the_guard_too() {
    let home = Home::new();
    let program = Program::new();
    let guard = waiting("acme-policy", "deny", "tools = [\"peek\"]");
    let (core, peek, path) = scripted(&home, &program, &guard).await;
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    assert!(peek.calls().is_empty(), "脚本里调的也挡下");
    let asked = asked(&path, "tool.guard");
    assert_eq!(asked.len(), 1);
    assert!(
        asked[0]["call_id"]
            .as_str()
            .is_some_and(|id| id.contains('#')),
        "嵌套调用的编号：{}",
        asked[0]
    );
    let shell_result = text_of(&results(&home, &session)[0]);
    assert!(
        shell_result.contains("Blocked by acme-policy: no writes here"),
        "{shell_result}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn an_interrupt_also_cancels_a_guard_asked_from_a_script() {
    let home = Home::new();
    let program = Program::new();
    let guard = waiting(
        "acme-policy",
        "silent",
        "tools = [\"peek\"]\ntimeout_ms = 5000",
    );
    let (core, peek, path) = scripted(&home, &program, &guard).await;
    let (mut client, session) = talk(&core).await;
    until_seen(&path, |text| text.contains("tool.guard")).await;
    let begun = Instant::now();
    let reply = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "keep"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    until_seen(&path, |text| text.contains("guard.cancel")).await;
    assert!(
        begun.elapsed() < Duration::from_secs(4),
        "父级被打断、嵌套调用了结时当场发，不等到超时"
    );
    assert!(peek.calls().is_empty());
    let asked = asked(&path, "tool.guard");
    let cancelled = crate::tool_guard::asked(&path, "guard.cancel");
    assert_eq!(cancelled[0]["call_id"], asked[0]["call_id"]);
    core.stop_extensions().await;
}
