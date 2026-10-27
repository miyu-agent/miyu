//! 越过沙盒的放行规则照命令前缀记（施工 SB-3，终端第 23 条，`docs/blueprint/session/guard.md` 第四条、第五条）：命令越过沙盒问人时，
//! 「此目录不再询问」的规则带上工具报的前缀；选了的，这个会话在这个目录（连同下面的）里同一前缀的命令越过沙盒不再问。前缀不同、
//! 工具没报前缀（连起来的几段不一样、包一层的、开头有赋值的）、工作目录换到外面的，照旧问。前缀怎么取在
//! `miyu-policy` 的 `hazard/prefix`；真经 `shell` 的那一条看它接上了。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::{ApprovalRequested, Body, Decision, Event, Level, Permission};
use miyu_kernel::session::{Answer, Command};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 调一次要在沙盒外跑的假工具 `run`，报前缀 `prefix`（没有的不报）。
fn run(command: &str, prefix: Option<&str>) -> Play {
    let mut args = serde_json::json!({
        "command": command, "title": "Run", "outside_sandbox": true
    });
    if let Some(prefix) = prefix {
        args["prefix"] = serde_json::json!(prefix);
    }
    Play::calls(&[("run", &args.to_string())])
}

/// 场地里的一个目录，建好、换成真实的位置。
fn dir(home: &Home, name: &str) -> PathBuf {
    let path = home.scratch.0.join(name);
    std::fs::create_dir_all(&path).expect("建得了");
    std::fs::canonicalize(&path).expect("在")
}

/// 在 `work/` 里造一个工作区级的会话，只有一件 `run`，沙盒能用。
async fn session(home: &Home, script: &Script, run: &Arc<Fake>) -> Handle {
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: true,
        cwd: dir(home, "work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
        sandbox_cache: None,
    };
    let catalog = Catalog::new([Arc::clone(run) as Arc<dyn Tool>]).expect("合写法");
    home.create_as(script, &catalog, opening).await
}

fn requests(log: &[Event]) -> Vec<ApprovalRequested> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ApprovalRequested(request) => Some(request.clone()),
            _ => None,
        })
        .collect()
}

/// 等到日志里有第 `n` 个确认请求，交回它。
async fn nth_request(home: &Home, handle: &Handle, n: usize) -> ApprovalRequested {
    let session = handle.id().clone();
    until_logged(home, &session, |log| requests(log).len() >= n).await;
    requests(&home.log(&session)).remove(n - 1)
}

async fn answer(handle: &Handle, id: &str, request: &ApprovalRequested, decision: Decision) {
    let answer = Answer::Approval {
        decision,
        reason: None,
        attachments: Vec::new(),
        nested: None,
    };
    let call_id = request.call_id;
    ask(handle, id, Command::Answer { call_id, answer })
        .await
        .expect("会话在跑");
}

/// 头一次问：提的规则带前缀，选「此目录不再询问」。
async fn grant_first(home: &Home, handle: &Handle, prefix: &str) {
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let first = nth_request(home, handle, 1).await;
    let work = dir(home, "work");
    assert_eq!(
        first.rule.as_ref().map(|rule| rule.get().to_string()),
        Some(
            serde_json::json!({ "prefix": prefix, "tool": "run", "write": [work.to_string_lossy()] })
                .to_string()
        ),
        "提的规则带上前缀"
    );
    answer(handle, "cmd-2", &first, Decision::Session).await;
}

#[tokio::test]
async fn the_same_prefix_here_is_not_asked_again() {
    let home = Home::outside_temp();
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([
        run("docker ps", Some("docker ps")),
        run("docker ps -a", Some("docker ps")),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, &fake).await;
    let mut pushes = watch(&handle).await;
    grant_first(&home, &handle, "docker ps").await;
    until_turn_ends(&mut pushes).await;
    let calls = fake.calls();
    assert_eq!(calls.len(), 2, "两次都跑了");
    assert!(calls[1].sandbox.is_none(), "第二次照样在沙盒外跑");
    assert_eq!(
        requests(&home.log(handle.id())).len(),
        1,
        "同一前缀、同一目录不再问"
    );
}

#[tokio::test]
async fn another_prefix_or_none_is_asked_again() {
    for second in [
        run("npm run build", Some("npm run")),
        // 工具没报前缀：连起来的几段不一样、包一层的、开头有赋值的都是这样。
        run("docker ps | grep x", None),
    ] {
        let home = Home::outside_temp();
        let fake = Fake::new("run", Access::Execute, Act::Echo);
        let script = Script::new([
            run("docker ps", Some("docker ps")),
            second,
            Play::Says("好。"),
        ]);
        let handle = session(&home, &script, &fake).await;
        let mut pushes = watch(&handle).await;
        grant_first(&home, &handle, "docker ps").await;
        let again = nth_request(&home, &handle, 2).await;
        answer(&handle, "cmd-3", &again, Decision::Deny).await;
        until_turn_ends(&mut pushes).await;
        assert_eq!(fake.calls().len(), 1, "第二次照旧问，拒了没跑");
        stop(&handle).await;
    }
}

#[tokio::test]
async fn below_the_folder_it_holds_and_outside_it_asks() {
    let home = Home::outside_temp();
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([
        run("docker ps", Some("docker ps")),
        Play::Says("好。"),
        run("docker ps", Some("docker ps")),
        Play::Says("好。"),
        run("docker ps", Some("docker ps")),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, &fake).await;
    let mut pushes = watch(&handle).await;
    grant_first(&home, &handle, "docker ps").await;
    until_turn_ends(&mut pushes).await;
    // 换到下面的目录：不问。
    let sub = dir(&home, "work/sub");
    let below = Command::SetWorkspace {
        cwd: sub.to_string_lossy().into_owned(),
        dirs: None,
    };
    ask(&handle, "cmd-3", below).await.expect("换得了");
    ask(&handle, "cmd-4", say("again")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(fake.calls().len(), 2, "下面的目录里跑了");
    assert_eq!(requests(&home.log(handle.id())).len(), 1, "下面的目录不问");
    // 换到外面的目录：问。
    let other = dir(&home, "other");
    let outside = Command::SetWorkspace {
        cwd: other.to_string_lossy().into_owned(),
        dirs: None,
    };
    ask(&handle, "cmd-5", outside).await.expect("换得了");
    ask(&handle, "cmd-6", say("once more"))
        .await
        .expect("会话在跑");
    let again = nth_request(&home, &handle, 2).await;
    answer(&handle, "cmd-7", &again, Decision::Deny).await;
    until_turn_ends(&mut pushes).await;
    assert_eq!(fake.calls().len(), 2, "外面的目录照旧问，拒了没跑");
}

/// 真经 `shell`：它报的前缀接上了。连起来的两段都是 `echo` 的放行以后不问；前面包了 `env` 的照问。
#[cfg(unix)]
#[tokio::test]
async fn shell_reports_its_prefix() {
    let home = Home::outside_temp();
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let call = |command: &str| {
        let args = serde_json::json!({
            "command": command, "description": "Say", "outside_sandbox": true
        });
        Play::calls(&[("shell", &args.to_string())])
    };
    let script = Script::new([
        call("echo one"),
        call("echo two && echo three"),
        call("env echo four"),
        Play::Says("好。"),
    ]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: true,
        cwd: dir(&home, "work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let first = nth_request(&home, &handle, 1).await;
    let rule: serde_json::Value =
        serde_json::from_str(first.rule.as_ref().expect("提了规则").get()).expect("是 JSON");
    assert_eq!(rule["prefix"], "echo");
    answer(&handle, "cmd-2", &first, Decision::Session).await;
    let wrapped = nth_request(&home, &handle, 2).await;
    let detail = wrapped.detail.as_ref().expect("有说明").get().to_string();
    assert!(
        detail.contains("env echo four"),
        "问的是包了 env 的那一条：{detail}"
    );
    assert!(
        wrapped
            .rule
            .as_ref()
            .is_some_and(|rule| !rule.get().contains("prefix")),
        "包了一层的不报前缀：{:?}",
        wrapped.rule
    );
    answer(&handle, "cmd-3", &wrapped, Decision::Deny).await;
    until_turn_ends(&mut pushes).await;
}
