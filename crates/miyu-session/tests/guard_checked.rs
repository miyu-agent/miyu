//! 工具只碰判过的位置（施工 5-18，`docs/blueprint/session/tools.md` 第 1 条）：权限策略判的时候换成的真实位置，执行器照调用
//! 交给工具（`Call::checked`），直接调用、脚本里调的都一样；判完以后路径上的链接被换掉的，工具换出来的不是判过的，不碰。
//!
//! 场地照 `guard.rs`：数据根 `data/`、假的家 `home/`、工作区 `work/`、边界以外的 `other/`。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 场地里的一处，写成她会给的样子（绝对路径）。
fn at(home: &Home, path: &str) -> String {
    home.scratch.0.join(path).to_string_lossy().into_owned()
}

/// 场地里的一处的真实位置。
fn real(home: &Home, path: &str) -> PathBuf {
    std::fs::canonicalize(home.scratch.0.join(path)).expect("在")
}

fn file(home: &Home, path: &str) {
    let path = home.scratch.0.join(path);
    std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(path, "x").expect("写得进");
}

/// 在工作区 `work/` 里造一个会话：工作区一级，有人能确认，工具是 `tools`。
async fn session(home: &Home, script: &Script, tools: &Catalog) -> miyu_session::Handle {
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: true,
        cwd: at(home, "work"),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    home.create_as(script, tools, opening).await
}

/// 说一句，等这一轮说完。
async fn turn(handle: &miyu_session::Handle) {
    let mut pushes = watch(handle).await;
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

fn catalog(tools: &[&Arc<Fake>]) -> Catalog {
    Catalog::new(tools.iter().map(|tool| Arc::clone(tool) as Arc<dyn Tool>)).expect("合写法")
}

#[tokio::test]
async fn the_tool_gets_the_positions_the_guard_judged() {
    let home = Home::outside_temp();
    file(&home, "work/a.txt");
    file(&home, "other/b.txt");
    let look = Fake::new("look", Access::Read, Act::Echo);
    let run = Fake::new("run", Access::Read, Act::Echo);
    let paths = serde_json::json!({"paths": ["a.txt", at(&home, "other/b.txt"), "new.txt"]});
    let script = Script::new([
        Play::calls(&[("look", &paths.to_string()), ("run", "{}")]),
        Play::Says("好了。"),
    ]);
    let handle = session(&home, &script, &catalog(&[&look, &run])).await;
    turn(&handle).await;
    let [call] = look.calls().try_into().expect("跑了一次");
    let expected = vec![
        real(&home, "work/a.txt"),
        real(&home, "other/b.txt"),
        real(&home, "work").join("new.txt"),
    ];
    assert_eq!(call.checked.as_deref(), Some(expected.as_slice()));
    // 不报路径的：一个都没判过，照样带上，碰什么都不行。
    let [call] = run.calls().try_into().expect("跑了一次");
    assert_eq!(call.checked.as_deref(), Some(&[][..]));
}

/// 脚本里调的（施工 M-2）照同一条链判，判过的位置一样交给工具。
#[tokio::test]
async fn a_nested_call_gets_the_positions_the_guard_judged() {
    let home = Home::outside_temp();
    file(&home, "work/a.txt");
    let args = serde_json::json!({"path": at(&home, "work/a.txt")}).to_string();
    let args: &'static str = Box::leak(args.into_boxed_str());
    let shell = Fake::commanding(
        "shell",
        Access::Read,
        Some(miyu_tool::RUN_IN_BACKGROUND),
        Act::Calls(vec![("look", args)]),
    );
    let look = Fake::callable("look", Access::Read, Act::Echo);
    let script = Script::new([
        Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
        Play::Says("好了。"),
    ]);
    let handle = session(&home, &script, &catalog(&[&shell, &look])).await;
    turn(&handle).await;
    let [call] = look.calls().try_into().expect("脚本里调了一次");
    assert_eq!(
        call.checked.as_deref(),
        Some(&[real(&home, "work/a.txt")][..])
    );
}

/// 审计复现（漏洞排查 2026-10-22）：`write` 写工作区里的链接 `link/` 下面，链接指到边界以外，权限策略照外面判、问人；人还没点，
/// 链接被换成指进数据根（沙盒里的命令就换得了工作区里的链接）。人点了允许，`write` 换出来的落在数据根里，不是判过的那一个：
/// 不写，说路径在判过以后变了。修之前写进了数据根。
#[cfg(unix)]
#[tokio::test]
async fn a_link_swapped_while_asking_is_not_followed_into_the_data_root() {
    use miyu_kernel::event::{Body, Decision, ToolStatus};
    use miyu_kernel::session::{Answer, Command};
    use std::os::unix::fs::symlink;

    let home = Home::outside_temp();
    let link = home.scratch.0.join("work/link");
    symlink(home.scratch.0.join("other"), &link).expect("造得了链接");
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let args = serde_json::json!({"file_path": "link/planted.txt", "content": "x"});
    let script = Script::new([
        Play::calls(&[("write", &args.to_string())]),
        Play::Says("好了。"),
    ]);
    let handle = session(&home, &script, &tools).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let session_id = handle.id().clone();
    until_logged(&home, &session_id, |log| {
        kinds(log).contains(&"tool.approval_requested")
    })
    .await;
    let call_id = home
        .log(&session_id)
        .iter()
        .find_map(|event| match &event.body {
            Body::ApprovalRequested(request) => Some(request.call_id),
            _ => None,
        })
        .expect("问了人");
    std::fs::remove_file(&link).expect("删得掉链接");
    symlink(home.scratch.0.join("data"), &link).expect("造得了链接");
    let answer = Answer::Approval {
        decision: Decision::Once,
        reason: None,
        attachments: Vec::new(),
        nested: None,
    };
    ask(&handle, "cmd-2", Command::Answer { call_id, answer })
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let result = home
        .log(&session_id)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一条工具结果");
    let text: String = result
        .blocks
        .iter()
        .filter_map(|block| match block {
            miyu_kernel::block::Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(result.status, ToolStatus::Error, "{text}");
    assert!(
        text.contains("changed after the permission check"),
        "{text}"
    );
    assert!(
        !home.scratch.0.join("data/planted.txt").exists(),
        "写进了数据根"
    );
    assert!(!home.scratch.0.join("other/planted.txt").exists());
}
