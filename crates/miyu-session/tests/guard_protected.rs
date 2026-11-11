//! 受保护的路径（只禁写的名单，施工 5-17 下）撤了（施工 LV-5，设计 11 的 A17）：以前名单上的（工作区里的 `.git/hooks`、
//! `.git/config`、编辑器的配置这些）和旁边平常的文件判的一样。撤之前这份测试还带着三种配置（关掉、加一项、去掉一项）跑过，
//! 判的都一样：A16 起只剩完全放开、只读两级，名单在判法表里已经不产生差别。

use std::sync::Arc;

use miyu_kernel::event::{Body, Level, Permission, ToolStatus};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 在场地的 `work` 里造一个会话（只读 `read_only`，有人能确认），调一次 `tool`，参数里是路径 `path`；交回问没问人、
/// 第一条工具结果的状态（问了人、还没有结果的没有）。
async fn judged(
    home: &Home,
    read_only: bool,
    (tool, path): (&str, &str),
) -> (bool, Option<ToolStatus>) {
    let fakes = [
        Fake::new("read", Access::Read, Act::Echo),
        Fake::new("edit", Access::Write, Act::Echo),
    ];
    let catalog = Catalog::new(fakes.map(|tool| tool as Arc<dyn Tool>)).expect("合写法");
    let path = home.scratch.0.join(path).to_string_lossy().into_owned();
    let args = serde_json::json!({ "path": path }).to_string();
    let script = Script::new([Play::calls(&[(tool, args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only,
        },
        attended: true,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
    };
    let handle = home.create_as(&script, &catalog, opening).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        let kinds = kinds(log);
        kinds.contains(&"tool.approval_requested") || kinds.contains(&"turn.ended")
    })
    .await;
    let log = home.log(handle.id());
    let asked = log
        .iter()
        .any(|event| matches!(event.body, Body::ApprovalRequested(_)));
    let status = log.into_iter().find_map(|event| match event.body {
        Body::ToolResult(result) => Some(result.status),
        _ => None,
    });
    (asked, status)
}

#[tokio::test]
async fn once_listed_paths_are_judged_like_plain_files() {
    const PLAIN: &str = "work/src/a.rs";
    const LISTED: [&str; 4] = [
        "work/.git/hooks/pre-commit",
        "work/.git/config",
        "work/.vscode/tasks.json",
        "work/Makefile",
    ];
    let home = Home::outside_temp();
    for path in LISTED.iter().chain([&PLAIN]) {
        let path = home.scratch.0.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, "x").expect("写得进");
    }
    for read_only in [false, true] {
        for tool in ["read", "edit"] {
            let plain = judged(&home, read_only, (tool, PLAIN)).await;
            let expected = match (read_only, tool) {
                (true, "edit") => (false, Some(ToolStatus::Denied)),
                _ => (false, Some(ToolStatus::Ok)),
            };
            assert_eq!(plain, expected, "只读 {read_only} {tool}");
            for listed in LISTED {
                assert_eq!(
                    judged(&home, read_only, (tool, listed)).await,
                    plain,
                    "只读 {read_only} {tool} {listed}"
                );
            }
        }
    }
}
