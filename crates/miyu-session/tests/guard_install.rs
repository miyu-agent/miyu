//! 改装了什么的（访问类别 `install`，施工 F-10 下，`docs/blueprint/tools/package.md`）：完全放开每次问人、不提规则，没人能确认
//! 的拒绝（施工 LV-1，2026-11-09 定：装扩展等于在这台机器上跑别人的程序，同 `curl | sh` 归高危，原来工作区一级的做法挪到完全
//! 放开）；只读拒绝，不问人。报的路径照判：数据根里的哪一级都拒。

use std::sync::Arc;

use miyu_kernel::event::{Body, Level, Permission, ToolStatus};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 在工作区 `work/` 里造一个完全放开的会话，目录里只有一件改装了什么的假工具 `package`：只读 `read_only`、有没有人能确认
/// `attended`。
async fn session(
    home: &Home,
    script: &Script,
    tool: &Arc<Fake>,
    (read_only, attended): (bool, bool),
) -> Handle {
    let catalog = Catalog::new([Arc::clone(tool) as Arc<dyn Tool>]).expect("合写法");
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only,
        },
        attended,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
    };
    home.create_as(script, &catalog, opening).await
}

/// 调一次 `package`，参数 `args`；说一句，等这一轮说完（问人的等到请求落了盘）。
async fn called(home: &Home, args: &str, level: (bool, bool)) -> (Arc<Fake>, Handle) {
    let tool = Fake::new("package", Access::Install, Act::Echo);
    let script = Script::new([Play::calls(&[("package", args)]), Play::Says("好。")]);
    let (read_only, attended) = level;
    let asks = attended && !read_only;
    let handle = session(home, &script, &tool, level).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    if asks {
        let id = handle.id().clone();
        until_logged(home, &id, |log| {
            kinds(log).contains(&"tool.approval_requested")
        })
        .await;
    } else {
        until_turn_ends(&mut pushes).await;
    }
    (tool, handle)
}

#[tokio::test]
async fn full_access_asks_every_time_without_a_rule() {
    let home = Home::outside_temp();
    let (tool, handle) = called(&home, r#"{"remove":"kit"}"#, (false, true)).await;
    assert!(tool.calls().is_empty(), "还没回答，没跑");
    let request = home
        .log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ApprovalRequested(request) => Some(request),
            _ => None,
        })
        .expect("有一条确认请求");
    assert_eq!(request.access, Access::Install);
    assert!(request.rule.is_none(), "不提规则：每次都问");
}

#[tokio::test]
async fn nobody_to_ask_or_read_only_refuses_it() {
    for (attended, read_only) in [(false, false), (true, true)] {
        let home = Home::outside_temp();
        let (tool, handle) = called(&home, r#"{"remove":"kit"}"#, (read_only, attended)).await;
        let label = format!("没人能确认 {}、只读 {read_only}", !attended);
        assert!(tool.calls().is_empty(), "{label}：没跑");
        let log = home.log(handle.id());
        assert!(
            !kinds(&log).contains(&"tool.approval_requested"),
            "{label}：不问"
        );
        let status = log.into_iter().find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result.status),
            _ => None,
        });
        assert_eq!(status, Some(ToolStatus::Denied), "{label}");
    }
}

#[tokio::test]
async fn a_folder_in_the_data_root_is_refused_even_with_full_access() {
    let home = Home::outside_temp();
    let inside = home.root.path().join("home").to_string_lossy().into_owned();
    let args = serde_json::json!({ "path": inside }).to_string();
    let (tool, _) = called(&home, &args, (false, false)).await;
    assert!(tool.calls().is_empty(), "数据根哪一级都不许");
}

/// 工作区里的包文件夹：报的路径照判是放行的，照级别还是要问（完全放开每次问）。
#[tokio::test]
async fn a_folder_in_the_workspace_still_asks() {
    let home = Home::outside_temp();
    let inside = home
        .scratch
        .0
        .join("work/kit")
        .to_string_lossy()
        .into_owned();
    let args = serde_json::json!({ "path": inside }).to_string();
    let (tool, handle) = called(&home, &args, (false, true)).await;
    assert!(tool.calls().is_empty(), "还没回答，没跑");
    assert!(kinds(&home.log(handle.id())).contains(&"tool.approval_requested"));
}
