//! 工作目录和工作区分开（施工 5-14，`docs/designs/11-权限与沙盒.md` 第四节）：工作目录太宽（系统的家目录）的会话，命令照旧
//! 从那里起，能写的是这个会话的默认工作区 `home/<账号>/workspace/<会话>/`，用到时建好；子会话、子会话的子会话用主会话的
//! 那一个，`session.created` 记下 `workspace_from`；载入以后权限策略照样只让写那里。场地的假家目录是 `home/`。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, ToolStatus};
use miyu_kernel::id::SessionId;
use miyu_kernel::tool::Access;
use miyu_sandbox::Sandboxed;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, Lineage};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 真实的位置。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("在")
}

/// 会话 `session` 的默认工作区。
fn own(home: &Home, session: &SessionId) -> PathBuf {
    home.root.session_workspace(&alice_account(), session)
}

/// 在家目录里造一个会话（工作区这一级，沙盒能用：假的助手，假工具不起它），父会话照 `lineage`：执行命令的假工具 `run`
/// 跑一次，交回会话和它拿到的沙盒。
async fn in_home(home: &Home, lineage: Option<Lineage>) -> (Handle, Arc<Sandboxed>) {
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: home.home.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
        sandbox_cache: None,
    };
    let lines = Lines {
        lineage,
        ..Lines::default()
    };
    let handle = home.create_full(&script, &tools, opening, lines).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let calls = run.calls();
    assert_eq!(calls.len(), 1, "跑了一次");
    assert_eq!(
        Path::new(&calls[0].cwd),
        home.home.as_path(),
        "命令照旧从工作目录起"
    );
    (handle, calls[0].sandbox.clone().expect("进了沙盒"))
}

/// 日志里 `session.created` 记的 `workspace_from`。
fn workspace_from(home: &Home, session: &SessionId) -> Option<SessionId> {
    home.log(session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::SessionCreated(created) => Some(created.workspace_from),
            _ => None,
        })?
}

#[tokio::test]
async fn a_too_wide_directory_writes_only_in_the_session_s_own_folder() {
    let home = Home::outside_temp();
    let (handle, sandboxed) = in_home(&home, None).await;
    let mine = own(&home, handle.id());
    assert!(mine.is_dir(), "用到时建好了：{}", mine.display());
    assert_eq!(
        sandboxed.spec.write[0],
        real(&mine),
        "{:?}",
        sandboxed.spec.write
    );
    assert!(
        !sandboxed.spec.write.contains(&real(&home.home)),
        "家目录不能写：{:?}",
        sandboxed.spec.write
    );
    assert_eq!(
        workspace_from(&home, handle.id()),
        None,
        "主会话用自己的，不写"
    );
}

#[tokio::test]
async fn children_write_in_the_main_session_s_folder() {
    let home = Home::outside_temp();
    let (main, _) = in_home(&home, None).await;
    let parent = main.id().clone();
    let (child, sandboxed) = in_home(
        &home,
        Some(Lineage {
            parent: parent.clone(),
            depth: 1,
            workspace_from: None,
        }),
    )
    .await;
    assert_eq!(workspace_from(&home, child.id()), Some(parent.clone()));
    assert_eq!(sandboxed.spec.write[0], real(&own(&home, &parent)));
    // 子会话的子会话：照子会话记的那一个，追到主会话。
    let (grandchild, sandboxed) = in_home(
        &home,
        Some(Lineage {
            parent: child.id().clone(),
            depth: 2,
            workspace_from: Some(parent.clone()),
        }),
    )
    .await;
    assert_eq!(workspace_from(&home, grandchild.id()), Some(parent.clone()));
    assert_eq!(sandboxed.spec.write[0], real(&own(&home, &parent)));
    assert!(!own(&home, child.id()).exists(), "子会话没有自己的那一个");
}

/// 一次调 `tool`，参数里是路径 `path`。
fn one(tool: &str, path: &Path) -> Play {
    Play::calls(&[(
        tool,
        &serde_json::json!({ "path": path.to_string_lossy() }).to_string(),
    )])
}

/// 日志里每一条工具结果的状态。
fn statuses(log: &[Event]) -> Vec<ToolStatus> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.status.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn after_a_reload_file_tools_still_write_only_in_that_folder() {
    let home = Home::outside_temp();
    let edit = Fake::new("edit", Access::Write, Act::Echo);
    let catalog = Catalog::new([Arc::clone(&edit) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::Says("嗯。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: home.home.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &catalog, opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    let mine = own(&home, handle.id());
    let again = Script::new([
        one("edit", &mine.join("notes.md")),
        one("edit", &home.home.join("notes.md")),
        Play::Says("好。"),
    ]);
    let cwd = home.home.to_string_lossy().into_owned();
    let loaded = home
        .load_full(handle.id(), &again, &catalog, &cwd, None)
        .await;
    let mut pushes = watch(&loaded).await;
    ask(&loaded, "cmd-2", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(handle.id());
    assert_eq!(
        statuses(&log),
        [ToolStatus::Ok, ToolStatus::Denied],
        "会话的那一个放行，家目录别处要问（没人能确认，拒）：{log:#?}"
    );
    assert_eq!(edit.calls().len(), 1);
}

/// 真的经助手跑 `shell`（Unix 上有收紧手段的）：命令在家目录里起，家目录里写不了，会话的那一个写得了。
#[cfg(unix)]
#[tokio::test]
async fn a_real_command_starts_at_home_and_writes_in_the_session_s_folder() {
    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, std::time::Duration::from_secs(60)).expect("探得了");
    if probe.mechanisms.is_empty() {
        return;
    }
    let home = Home::outside_temp();
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    // 先编好会话的编号：命令里要写它的默认工作区。
    let session = miyu_session::new_id(now());
    let mine = own(&home, &session);
    let command = format!(
        "pwd -P; touch '{}' 2>/dev/null && echo wrote-home || echo blocked-home; touch '{}' && echo wrote-own",
        home.home.join("a.txt").display(),
        mine.join("b.txt").display(),
    );
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let script = Script::new([Play::calls(&[("shell", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: home.home.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(helper),
        sandbox_cache: None,
    };
    let lines = Lines {
        id: Some(session.clone()),
        ..Lines::default()
    };
    let handle = home.create_full(&script, &tools, opening, lines).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let result = home
        .log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一条工具结果");
    assert_eq!(result.status, ToolStatus::Ok, "{result:?}");
    let text: String = result
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect();
    assert_eq!(
        text,
        format!("{}\nblocked-home\nwrote-own\n", real(&home.home).display())
    );
    assert!(!home.home.join("a.txt").exists());
}
