//! 高危命令（施工 5-17，`docs/blueprint/session/guard.md`）：真的 `shell`，完全放开里灭顶的直接拒（她拿到那一句，人看不到
//! 确认）；别的问人（说明多 `hazard`，不提规则），没人能确认的拒（A16 起完全放开也照问，施工 LV-1，改掉 5-17 再补的「完全放开
//! 不问」）；只读不看它。设置页「受保护的命令」关掉的那一类不问、直接跑，下一轮生效（施工 HZ-1，设计 11 的 A18）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_kernel::event::{
    ApprovalRequested, Body, Event, Level, Permission, ToolResult, ToolStatus,
};
use miyu_session::testkit::{Play, Script};
use miyu_session::{ConfigSource, Handle};
use miyu_tool::Catalog;
use tokio::sync::watch;

use crate::support::*;

/// 真的基础系统，`shell` 在里面。
fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 在完全放开、只读 `read_only`、有没有人能确认 `attended` 的会话里让她跑一次 `command`（沙盒能用：假的助手，拒了、问了都走
/// 不到它）。
async fn ran(home: &Home, (read_only, attended): (bool, bool), command: &str) -> Handle {
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let script = Script::new([Play::calls(&[("shell", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only,
        },
        attended,
        cwd: home.home.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
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
async fn wiping_out_home_is_refused_even_with_full_access() {
    let home = Home::outside_temp();
    let handle = ran(&home, (false, true), "rm -rf ~").await;
    let log = home.log(handle.id());
    assert!(requests(&log).is_empty(), "不问");
    let result = results(&log).remove(0);
    assert_eq!(result.status, ToolStatus::Denied);
    let text = format!("{:?}", result.blocks);
    assert!(
        text.contains(
            "Not run: this command would delete the home directory, which is never allowed."
        ),
        "{text}"
    );
}

#[tokio::test]
async fn a_force_push_asks_without_a_rule_with_full_access() {
    let home = Home::outside_temp();
    let handle = ran(&home, (false, true), "git push --force").await;
    let request = requests(&home.log(handle.id())).remove(0);
    assert_eq!(request.rule, None, "高危的不该一劳永逸");
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

/// 完全放开也问问人的那几种（A16，施工 LV-1，2026-11-09 项目主人定：不做沙盒不等于不做保护，改掉 5-17 再补的「完全放开只拦
/// 灭顶的」）：有人能确认的问、不提规则；没人能确认的拒，没跑。
#[tokio::test]
async fn full_asks_for_the_asking_kinds_too() {
    let home = Home::outside_temp();
    let handle = ran(&home, (false, true), "git stash drop").await;
    let log = home.log(handle.id());
    let request = requests(&log).remove(0);
    assert_eq!(request.rule, None, "高危的不该一劳永逸");
    let detail = request
        .detail
        .as_ref()
        .map(|detail| detail.get().to_string())
        .unwrap_or_default();
    assert!(detail.contains(r#""hazard":"git_stash_drop""#), "{detail}");
    assert!(results(&log).is_empty(), "还没回答，没跑：{log:#?}");
    let home = Home::outside_temp();
    let handle = ran(&home, (false, false), "git stash drop").await;
    let log = home.log(handle.id());
    assert!(requests(&log).is_empty(), "没人能确认：不问");
    assert_eq!(results(&log).remove(0).status, ToolStatus::Denied);
}

#[tokio::test]
async fn read_only_leaves_it_to_the_read_only_sandbox() {
    let home = Home::outside_temp();
    let handle = ran(&home, (true, true), "git push --force").await;
    assert!(requests(&home.log(handle.id())).is_empty(), "只读不看高危");
}

#[tokio::test]
async fn ordinary_commands_are_not_held_up() {
    let home = Home::outside_temp();
    let handle = ran(&home, (false, true), "git status").await;
    assert!(
        requests(&home.log(handle.id())).is_empty(),
        "完全放开照旧不问"
    );
}

/// 受保护的命令一类一个开关（施工 HZ-1，设计 11 的 A18）：`permission.confirm.git` 关着的那一轮，`git reset --hard` 不问就跑；
/// 改回开着，下一轮又问。命令指着一个没有的目录：真跑了也只是报错，碰不到谁的仓库。
#[tokio::test]
async fn a_group_switched_off_runs_without_asking_until_it_is_back_on() {
    let source = |text: &str| -> Arc<dyn ConfigSource> {
        Arc::clone(&*crate::support::routing::configs(text, &[]).borrow())
    };
    let (switch, receiving) = watch::channel(source("[permission.confirm]\ngit = false\n"));
    let mut home = Home::outside_temp();
    home.configs = receiving;
    let command = format!(
        "git -C {} reset --hard",
        home.home.join("no-such-repo").to_string_lossy()
    );
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let call = || Play::calls(&[("shell", args.as_str())]);
    let script = Script::new([call(), Play::Says("好。"), call(), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        attended: true,
        cwd: home.home.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
    };
    let handle = home.create_as(&script, &tools(), opening).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_logged(&home, handle.id(), |log| {
        kinds(log).contains(&"tool.approval_requested") || kinds(log).contains(&"turn.ended")
    })
    .await;
    let log = home.log(handle.id());
    assert!(requests(&log).is_empty(), "关了这一类：不问，{log:#?}");
    let result = results(&log).remove(0);
    assert_ne!(result.status, ToolStatus::Denied, "跑了：{result:?}");

    // 改回开着：下一轮又问。
    switch.send_replace(source("[permission.confirm]\ngit = true\n"));
    ask(&handle, "cmd-2", say("again")).await.expect("会话在跑");
    until_logged(&home, handle.id(), |log| {
        kinds(log).contains(&"tool.approval_requested")
            || kinds(log)
                .iter()
                .filter(|kind| **kind == "turn.ended")
                .count()
                > 1
    })
    .await;
    let log = home.log(handle.id());
    let request = requests(&log).pop().expect("改回来以后问了");
    let detail = request
        .detail
        .as_ref()
        .map(|detail| detail.get().to_string())
        .unwrap_or_default();
    assert!(detail.contains(r#""hazard":"git_reset_hard""#), "{detail}");
    assert_eq!(results(&log).len(), 1, "第二次还没回答，没跑");
}
