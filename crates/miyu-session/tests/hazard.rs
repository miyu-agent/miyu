//! 高危命令（施工 5-17，`docs/blueprint/session/guard.md`）：真的 `shell`，工作区、完全放开两级里灭顶的直接拒（她拿到那一句，
//! 人看不到确认），别的问人（说明多 `hazard`，不提规则）；只读不看它。

use std::path::{Path, PathBuf};

use miyu_kernel::event::{
    ApprovalRequested, Body, Event, Level, Permission, ToolResult, ToolStatus,
};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 真的基础系统，`shell` 在里面。
fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 在 `level` 级别的会话里让她跑一次 `command`（沙盒能用：假的助手，拒了、问了都走不到它）。
async fn ran(home: &Home, level: Level, read_only: bool, command: &str) -> Handle {
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let script = Script::new([Play::calls(&[("shell", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission { level, read_only },
        attended: true,
        cwd: home.home.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &tools(), opening).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        let kinds = kinds(log);
        kinds.contains(&"tool.approval_requested") || kinds.contains(&"tool.result")
    })
    .await;
    handle
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

#[tokio::test]
async fn wiping_out_home_is_refused_in_workspace_and_full_alike() {
    for level in [Level::Workspace, Level::Full] {
        let home = Home::outside_temp();
        let handle = ran(&home, level.clone(), false, "rm -rf ~").await;
        let log = home.log(handle.id());
        assert!(requests(&log).is_empty(), "不问：{level:?}");
        let result = results(&log).remove(0);
        assert_eq!(result.status, ToolStatus::Denied, "{level:?}");
        let text = format!("{:?}", result.blocks);
        assert!(
            text.contains("Not run: this command would delete the home directory."),
            "{text}"
        );
    }
}

#[tokio::test]
async fn a_force_push_asks_without_a_rule_even_in_full() {
    for level in [Level::Workspace, Level::Full] {
        let home = Home::outside_temp();
        let handle = ran(&home, level.clone(), false, "git push --force").await;
        let request = requests(&home.log(handle.id())).remove(0);
        assert_eq!(request.rule, None, "高危的不该一劳永逸：{level:?}");
        let detail = request
            .detail
            .as_ref()
            .map(|detail| detail.get().to_string())
            .unwrap_or_default();
        assert!(detail.contains(r#""hazard":"git_force_push""#), "{detail}");
        assert!(
            detail.contains(r#""command":"git push --force""#),
            "{detail}"
        );
    }
}

#[tokio::test]
async fn read_only_leaves_it_to_the_read_only_sandbox() {
    let home = Home::outside_temp();
    let handle = ran(&home, Level::Workspace, true, "git push --force").await;
    assert!(requests(&home.log(handle.id())).is_empty(), "只读不看高危");
}

#[tokio::test]
async fn ordinary_commands_are_not_held_up() {
    let home = Home::outside_temp();
    let handle = ran(&home, Level::Full, false, "git status").await;
    assert!(
        requests(&home.log(handle.id())).is_empty(),
        "完全放开照旧不问"
    );
}
