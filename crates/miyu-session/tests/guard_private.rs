//! 别的程序的私密目录（施工 5-13，`docs/blueprint/session/guard.md` 判法表）：文件工具读、写都问人，完全放开不问，只读时写
//! 照拒；问人的说明里这一片写 `private`，提的规则放行整个目录。工作区、加进来的目录落在里面的照人给的算。场地的假家目录
//! 是 `home/`。

use std::sync::Arc;

use miyu_kernel::event::{ApprovalRequested, Body, Level, Permission, ToolStatus};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 读（`read`）、写（`edit`）的假工具，报参数里的路径。
struct Kit {
    read: Arc<Fake>,
    edit: Arc<Fake>,
}

impl Kit {
    fn new() -> Kit {
        Kit {
            read: Fake::new("read", Access::Read, Act::Echo),
            edit: Fake::new("edit", Access::Write, Act::Echo),
        }
    }

    fn catalog(&self) -> Catalog {
        Catalog::new([&self.read, &self.edit].map(|tool| Arc::clone(tool) as Arc<dyn Tool>))
            .expect("合写法")
    }
}

/// 场地里的一处，写成她会给的样子（绝对路径）。
fn at(home: &Home, path: &str) -> String {
    home.scratch.0.join(path).to_string_lossy().into_owned()
}

fn file(home: &Home, path: &str) {
    let path = home.scratch.0.join(path);
    std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(path, "x").expect("写得进");
}

/// 在场地里的 `cwd` 造一个会话（级别 `level`、只读 `read_only`、有没有人能确认 `attended`，加进来的目录 `dirs`），调一次
/// `tool`，参数里是路径 `path`；等到确认请求或者这一轮结束落了盘。
async fn called(
    home: &Home,
    kit: &Kit,
    (cwd, dirs): (&str, Vec<String>),
    (level, read_only, attended): (Level, bool, bool),
    (tool, path): (&str, &str),
) -> Handle {
    let args = serde_json::json!({ "path": path }).to_string();
    let script = Script::new([Play::calls(&[(tool, args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission { level, read_only },
        attended,
        cwd: at(home, cwd),
        dirs,
        sandbox: None,
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &kit.catalog(), opening).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        let kinds = kinds(log);
        kinds.contains(&"tool.approval_requested") || kinds.contains(&"turn.ended")
    })
    .await;
    handle
}

/// 日志里的确认请求。
fn request(home: &Home, handle: &Handle) -> Option<ApprovalRequested> {
    home.log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ApprovalRequested(request) => Some(request),
            _ => None,
        })
}

/// 日志里第一条工具结果的状态。
fn status(home: &Home, handle: &Handle) -> ToolStatus {
    home.log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result.status),
            _ => None,
        })
        .expect("有一条工具结果")
}

#[tokio::test]
async fn a_read_in_a_private_dir_asks_and_says_it_is_private() {
    let home = Home::outside_temp();
    file(&home, "home/.ssh/id_ed25519");
    let key = at(&home, "home/.ssh/id_ed25519");
    for read_only in [false, true] {
        let kit = Kit::new();
        let handle = called(
            &home,
            &kit,
            ("work", Vec::new()),
            (Level::Workspace, read_only, true),
            ("read", &key),
        )
        .await;
        assert!(
            kit.read.calls().is_empty(),
            "只读 {read_only}：还没回答，没跑"
        );
        let request = request(&home, &handle).expect("问了人");
        let real = std::fs::canonicalize(&key).expect("在");
        let dir = std::fs::canonicalize(home.home.join(".ssh")).expect("在");
        let rule: serde_json::Value =
            serde_json::from_str(request.rule.expect("提了规则").get()).expect("是 JSON");
        assert_eq!(
            rule,
            serde_json::json!({"tool": "read", "read": [dir.to_string_lossy()]}),
            "放行整个目录"
        );
        let detail: serde_json::Value =
            serde_json::from_str(request.detail.expect("有说明").get()).expect("是 JSON");
        assert_eq!(
            detail,
            serde_json::json!({"tool": "read", "paths": [{"path": real.to_string_lossy(), "write": false, "zone": "private"}]})
        );
    }
}

#[tokio::test]
async fn nobody_to_ask_means_a_private_read_does_not_run() {
    let home = Home::outside_temp();
    file(&home, "home/.claude/settings.json");
    let kit = Kit::new();
    let handle = called(
        &home,
        &kit,
        ("work", Vec::new()),
        (Level::Workspace, false, false),
        ("read", &at(&home, "home/.claude/settings.json")),
    )
    .await;
    assert_eq!(status(&home, &handle), ToolStatus::Denied);
    assert!(kit.read.calls().is_empty());
}

#[tokio::test]
async fn read_only_refuses_a_private_write_without_asking() {
    let home = Home::outside_temp();
    file(&home, "home/.gnupg/pubring.kbx");
    let kit = Kit::new();
    let handle = called(
        &home,
        &kit,
        ("work", Vec::new()),
        (Level::Workspace, true, true),
        ("edit", &at(&home, "home/.gnupg/pubring.kbx")),
    )
    .await;
    assert!(request(&home, &handle).is_none(), "不问");
    assert_eq!(status(&home, &handle), ToolStatus::Denied);
    assert!(kit.edit.calls().is_empty());
}

#[tokio::test]
async fn full_access_reads_and_writes_private_dirs_without_asking() {
    let home = Home::outside_temp();
    file(&home, "home/.ssh/config");
    for tool in ["read", "edit"] {
        let kit = Kit::new();
        let handle = called(
            &home,
            &kit,
            ("work", Vec::new()),
            (Level::Full, false, false),
            (tool, &at(&home, "home/.ssh/config")),
        )
        .await;
        assert!(request(&home, &handle).is_none(), "{tool}：不问");
        assert_eq!(status(&home, &handle), ToolStatus::Ok, "{tool}");
    }
}

#[tokio::test]
async fn a_workspace_or_added_dir_inside_a_private_dir_goes_as_given() {
    let home = Home::outside_temp();
    file(&home, "home/.claude/proj/a.md");
    file(&home, "home/.config/gh/notes.md");
    let kit = Kit::new();
    let handle = called(
        &home,
        &kit,
        ("home/.claude/proj", Vec::new()),
        (Level::Workspace, false, false),
        ("edit", &at(&home, "home/.claude/proj/a.md")),
    )
    .await;
    assert_eq!(
        status(&home, &handle),
        ToolStatus::Ok,
        "工作区在里面：照工作区"
    );
    let kit = Kit::new();
    let handle = called(
        &home,
        &kit,
        ("work", vec![at(&home, "home/.config/gh")]),
        (Level::Workspace, false, false),
        ("edit", &at(&home, "home/.config/gh/notes.md")),
    )
    .await;
    assert_eq!(
        status(&home, &handle),
        ToolStatus::Ok,
        "加进来的目录：照工作区"
    );
}
