//! 越过沙盒问人放行一次（施工 D-4，`docs/blueprint/session/guard.md` 第四条、`tools/shell.md`）：工具报「这一次要在沙盒外跑」的
//! 调用，工作区这一级问人，提的规则是「此目录不再询问」：放行工作目录的写（施工 5-15，原来不提规则），允许了这一次不套沙盒，
//! 选了此目录不再询问的，以后的命令照旧在沙盒里跑、工作目录能写，越过沙盒的照样每次问；拒绝了她拿到平常的拒绝；只读不问、
//! 拒绝；完全放开不问、本来就不套。
//! 没报的照旧在沙盒里跑、不问。Unix 上有收紧手段的，真的经助手跑 `shell`：允许以后写得进工作区以外。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::{
    ApprovalRequested, Body, Decision, Event, Level, Permission, ToolResult, ToolStatus,
};
use miyu_kernel::session::{Answer, Command};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 假的助手：假工具不起它。
const HELPER: &str = "miyu-sandbox";

/// 调一次执行命令的假工具 `run`：`outside` 时要在沙盒外跑。
fn run(outside: bool) -> Play {
    let mut args =
        serde_json::json!({ "command": "touch ~/Downloads/x.txt", "title": "Create a file" });
    if outside {
        args["outside_sandbox"] = serde_json::json!(true);
    }
    Play::calls(&[("run", &args.to_string())])
}

/// 在场地的 `work/` 里造一个会话，只有一件 `run`，这台机器上的沙盒能用。
async fn session(
    home: &Home,
    script: &Script,
    run: &Arc<Fake>,
    level: Level,
    read_only: bool,
    attended: bool,
) -> Handle {
    let cwd = home.scratch.0.join("work").to_string_lossy().into_owned();
    let opening = Opening {
        permission: Permission { level, read_only },
        attended,
        cwd,
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from(HELPER)),
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

fn results(log: &[Event]) -> Vec<ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.clone()),
            _ => None,
        })
        .collect()
}

/// 说一句，等到日志里有确认请求，交回它。
async fn asked(home: &Home, handle: &Handle) -> ApprovalRequested {
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let session = handle.id().clone();
    until_logged(home, &session, |log| !requests(log).is_empty()).await;
    requests(&home.log(&session)).remove(0)
}

/// 工作目录（建好、换成真实的位置）放行写的那条规则的原文（施工 5-15）。
fn this_dir(home: &Home) -> String {
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let real = std::fs::canonicalize(&work).expect("在");
    serde_json::json!({ "tool": "run", "write": [real.to_string_lossy()] }).to_string()
}

async fn answer(handle: &Handle, request: &ApprovalRequested, decision: Decision) {
    let answer = Answer::Approval {
        decision,
        reason: None,
    };
    let call_id = request.call_id;
    ask(handle, "cmd-2", Command::Answer { call_id, answer })
        .await
        .expect("会话在跑");
}

#[tokio::test]
async fn allowing_once_runs_that_call_outside_the_sandbox() {
    let home = Home::outside_temp();
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([run(true), run(false), Play::Says("好。")]);
    let handle = session(&home, &script, &fake, Level::Workspace, false, true).await;
    let mut pushes = watch(&handle).await;
    let request = asked(&home, &handle).await;
    assert_eq!(request.access, Access::Execute);
    assert_eq!(
        request.rule.as_ref().map(|rule| rule.get().to_string()),
        Some(this_dir(&home)),
        "提的规则只放行工作目录的写，不放开整个沙盒"
    );
    assert_eq!(
        request.detail.as_ref().map(|detail| detail.get()),
        Some(
            r#"{"command":"touch ~/Downloads/x.txt","sandbox":false,"title":"Create a file","tool":"run"}"#
        )
    );
    answer(&handle, &request, Decision::Once).await;
    until_turn_ends(&mut pushes).await;
    let calls = fake.calls();
    assert_eq!(calls.len(), 2, "两次都跑了");
    assert!(calls[0].sandbox.is_none(), "允许了的那一次不套沙盒");
    assert!(calls[1].sandbox.is_some(), "没报的照旧在沙盒里跑");
    assert_eq!(requests(&home.log(handle.id())).len(), 1, "没报的不问");
}

#[tokio::test]
async fn not_asking_again_here_makes_the_folder_writable_in_the_sandbox_only() {
    // 在家目录里开的：工作区是会话自己的那一个（施工 5-14），放行的是家目录，两个不一样才看得出放行起了作用。
    let home = Home::outside_temp();
    let real_home = std::fs::canonicalize(&home.home).expect("在");
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([run(true), run(false), run(true), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: true,
        cwd: home.home.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from(HELPER)),
        sandbox_cache: None,
    };
    let catalog = Catalog::new([Arc::clone(&fake) as Arc<dyn Tool>]).expect("合写法");
    let handle = home.create_as(&script, &catalog, opening).await;
    let mut pushes = watch(&handle).await;
    let request = asked(&home, &handle).await;
    assert_eq!(
        request.rule.as_ref().map(|rule| rule.get().to_string()),
        Some(
            serde_json::json!({ "tool": "run", "write": [real_home.to_string_lossy()] })
                .to_string()
        ),
        "家目录本身也照放：人看得到放行的是哪个"
    );
    answer(&handle, &request, Decision::Session).await;
    // 第三次又要越过沙盒：照样问。
    let session_id = handle.id().clone();
    until_logged(&home, &session_id, |log| requests(log).len() >= 2).await;
    let call_id = requests(&home.log(&session_id)).remove(1).call_id;
    let deny = Answer::Approval {
        decision: Decision::Deny,
        reason: None,
    };
    ask(
        &handle,
        "cmd-3",
        Command::Answer {
            call_id,
            answer: deny,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let calls = fake.calls();
    assert_eq!(calls.len(), 2, "第三次拒了：{calls:?}");
    assert!(calls[0].sandbox.is_none(), "放行的那一次不套沙盒");
    let sandboxed = calls[1].sandbox.as_ref().expect("之后照旧在沙盒里跑");
    assert!(
        sandboxed.spec.write.contains(&real_home),
        "放行过的目录能写：{:?}",
        sandboxed.spec.write
    );
}

#[tokio::test]
async fn refusing_gives_her_the_normal_denial() {
    let home = Home::outside_temp();
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([run(true), Play::Says("好。")]);
    let handle = session(&home, &script, &fake, Level::Workspace, false, true).await;
    let mut pushes = watch(&handle).await;
    let request = asked(&home, &handle).await;
    answer(&handle, &request, Decision::Deny).await;
    until_turn_ends(&mut pushes).await;
    assert!(fake.calls().is_empty(), "拒绝了的没跑");
    let results = results(&home.log(handle.id()));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, ToolStatus::Denied, "{results:?}");
}

#[tokio::test]
async fn nobody_to_ask_means_no() {
    let home = Home::outside_temp();
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([run(true), Play::Says("好。")]);
    let handle = session(&home, &script, &fake, Level::Workspace, false, false).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert!(fake.calls().is_empty(), "没人能确认的不跑");
    let results = results(&home.log(handle.id()));
    assert_eq!(results[0].status, ToolStatus::Denied, "{results:?}");
}

#[tokio::test]
async fn read_only_refuses_without_asking() {
    let home = Home::outside_temp();
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([run(true), Play::Says("好。")]);
    let handle = session(&home, &script, &fake, Level::Workspace, true, true).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(handle.id());
    assert!(requests(&log).is_empty(), "只读不问");
    assert!(fake.calls().is_empty(), "只读不跑");
    let results = results(&log);
    assert_eq!(results[0].status, ToolStatus::Denied, "{results:?}");
}

#[tokio::test]
async fn full_runs_it_without_asking() {
    let home = Home::outside_temp();
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([run(true), Play::Says("好。")]);
    let handle = session(&home, &script, &fake, Level::Full, false, true).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert!(requests(&home.log(handle.id())).is_empty(), "完全放开不问");
    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].sandbox.is_none());
}

/// 拒绝的照拒：要在沙盒外跑也碰不了数据根，不问。
#[tokio::test]
async fn the_data_root_stays_out_of_reach() {
    let home = Home::outside_temp();
    let fake = Fake::new("edit", Access::Write, Act::Echo);
    let inside = home
        .root
        .path()
        .join("marker")
        .to_string_lossy()
        .into_owned();
    let args = serde_json::json!({ "path": inside, "outside_sandbox": true }).to_string();
    let script = Script::new([Play::calls(&[("edit", &args)]), Play::Says("好。")]);
    let handle = session(&home, &script, &fake, Level::Workspace, false, true).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(handle.id());
    assert!(requests(&log).is_empty(), "不问");
    assert!(fake.calls().is_empty());
    assert_eq!(results(&log)[0].status, ToolStatus::Denied);
}

/// 真的经助手跑 `shell`（Unix、有收紧手段的）：没写的写不进工作区以外；写了的问人、允许以后写得进。
#[cfg(unix)]
#[tokio::test]
async fn a_real_command_writes_outside_once_allowed() {
    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, std::time::Duration::from_secs(60)).expect("探得了");
    if probe.mechanisms.is_empty() {
        return;
    }
    let home = Home::outside_temp();
    let outside = home.scratch.0.join("other").join("x.txt");
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let command = format!(
        "(echo out > '{}') 2>/dev/null && echo wrote || echo blocked",
        outside.display()
    );
    let call = |outside_sandbox: bool| {
        let args = serde_json::json!({
            "command": command, "description": "Write outside", "outside_sandbox": outside_sandbox
        });
        Play::calls(&[("shell", &args.to_string())])
    };
    let script = Script::new([call(false), call(true), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: true,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(helper),
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    let mut pushes = watch(&handle).await;
    let request = asked(&home, &handle).await;
    answer(&handle, &request, Decision::Once).await;
    until_turn_ends(&mut pushes).await;
    let said: Vec<String> = results(&home.log(handle.id()))
        .iter()
        .map(|result| {
            result
                .blocks
                .iter()
                .map(|block| match block {
                    miyu_kernel::block::Block::Text(text) => text.text.clone(),
                    other => panic!("只该有字：{other:?}"),
                })
                .collect()
        })
        .collect();
    assert_eq!(said, ["blocked\n", "wrote\n"]);
    assert!(outside.exists());
}

/// 沙盒用不了的机器上每条命令都问（D-1 补）：说明照样带标题和命令，写明不在沙盒里跑。
#[tokio::test]
async fn without_a_sandbox_every_command_asks_with_its_title_and_command() {
    let home = Home::outside_temp();
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let script = Script::new([run(false), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: true,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let catalog = Catalog::new([Arc::clone(&fake) as Arc<dyn Tool>]).expect("合写法");
    let handle = home.create_as(&script, &catalog, opening).await;
    let mut pushes = watch(&handle).await;
    let request = asked(&home, &handle).await;
    assert_eq!(request.rule, None);
    assert_eq!(
        request.detail.as_ref().map(|detail| detail.get()),
        Some(
            r#"{"command":"touch ~/Downloads/x.txt","sandbox":false,"title":"Create a file","tool":"run"}"#
        )
    );
    answer(&handle, &request, Decision::Once).await;
    until_turn_ends(&mut pushes).await;
    assert_eq!(fake.calls().len(), 1);
}

/// 在家目录里放行了「此目录不再询问」（施工 5-15）：文件工具写家目录别处不再问，写私密目录照旧问。
#[tokio::test]
async fn a_granted_home_still_asks_before_a_private_dir() {
    let home = Home::outside_temp();
    std::fs::create_dir_all(home.home.join(".ssh")).expect("建得了");
    let fake = Fake::new("run", Access::Execute, Act::Echo);
    let edit = Fake::new("edit", Access::Write, Act::Echo);
    let path = |name: &str| home.home.join(name).to_string_lossy().into_owned();
    let script = Script::new([
        run(true),
        Play::calls(&[(
            "edit",
            &serde_json::json!({ "path": path("notes.md") }).to_string(),
        )]),
        Play::calls(&[(
            "edit",
            &serde_json::json!({ "path": path(".ssh/x") }).to_string(),
        )]),
        Play::Says("好。"),
    ]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: true,
        cwd: home.home.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from(HELPER)),
        sandbox_cache: None,
    };
    let catalog = Catalog::new([
        Arc::clone(&fake) as Arc<dyn Tool>,
        Arc::clone(&edit) as Arc<dyn Tool>,
    ])
    .expect("合写法");
    let handle = home.create_as(&script, &catalog, opening).await;
    let request = asked(&home, &handle).await;
    answer(&handle, &request, Decision::Session).await;
    let session_id = handle.id().clone();
    until_logged(&home, &session_id, |log| requests(log).len() >= 2).await;
    let again = requests(&home.log(&session_id)).remove(1);
    assert_eq!(again.access, Access::Write, "{again:?}");
    let detail = again
        .detail
        .as_ref()
        .map(|detail| detail.get().to_string())
        .unwrap_or_default();
    assert!(detail.contains("private"), "问的是私密目录那一次：{detail}");
    assert_eq!(edit.calls().len(), 1, "家目录别处的那一次没问就写了");
}
