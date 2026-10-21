//! 管理员事先放行出沙盒的工具（施工 X-36，`11-权限与沙盒.md` A15，`docs/blueprint/session/guard.md`）：系统配置的
//! `sandbox.tools.<名字>` 写的来处和工具目录里这件工具归的包一样，才算放行。放行了的，工作区这一级不问、执行器不给沙盒的规格；
//! 只读拒；只碰得到自己工作区的会话（群聊）拒；来处对不上的、没写的、文件是链接的（施工 X-37）照旧在沙盒里跑。脚本里调到的
//! （嵌套调用）走同一条链。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, ToolResult, ToolStatus};
use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::routing::configs;
use crate::support::*;

/// 假的助手：假工具不起它。
const HELPER: &str = "miyu-sandbox";

/// 系统配置：放行 `run`，写的来处是 `source`。
fn granting(source: &str) -> String {
    format!("[sandbox.tools]\nrun = \"{source}\"\n")
}

fn results(log: &[Event]) -> Vec<ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.clone()),
            _ => None,
        })
        .collect()
}

fn asked(log: &[Event]) -> bool {
    log.iter()
        .any(|event| matches!(event.body, Body::ApprovalRequested(_)))
}

/// 在场地的 `work/` 里造一个会话，系统配置是 `config`，工具照 `tools`（`~home` 的包里），沙盒能用；场所是 `venue`、属主是不是
/// 管理员照 `owner_is_admin`，这台机器上的沙盒能不能用照 `sandboxed`。她调一次 `call`，说一句，等这一轮完。
async fn turn(
    home: &mut Home,
    config: &str,
    tools: &[&Arc<Fake>],
    call: Play,
    (level, read_only): (Level, bool),
    (venue, owner_is_admin): (&str, bool),
    sandboxed: bool,
) -> Vec<Event> {
    home.configs = configs(config, &[]);
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作区");
    let opening = Opening {
        permission: Permission { level, read_only },
        attended: true,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: sandboxed.then(|| PathBuf::from(HELPER)),
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        owner_is_admin,
        ..Lines::default()
    };
    let catalog = Catalog::in_packages([(
        "~home",
        tools
            .iter()
            .map(|tool| Arc::clone(tool) as Arc<dyn Tool>)
            .collect(),
    )])
    .expect("合写法");
    let script = Script::new([call, Play::Says("好。")]);
    let handle = home.create_full(&script, &catalog, opening, lines).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    home.log(handle.id())
}

fn run() -> Arc<Fake> {
    Fake::new("run", Access::Execute, Act::Echo)
}

const WORKSPACE: (Level, bool) = (Level::Workspace, false);
const LOCAL: (&str, bool) = ("local", true);

#[tokio::test]
async fn a_granted_tool_runs_outside_the_sandbox_without_asking() {
    let mut home = Home::outside_temp();
    let fake = run();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(
        &mut home,
        &granting("~home"),
        &[&fake],
        call,
        WORKSPACE,
        LOCAL,
        true,
    )
    .await;
    assert!(!asked(&log), "放行了的不问");
    assert_eq!(results(&log)[0].status, ToolStatus::Ok);
    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].sandbox.is_none(), "不给沙盒的规格");
}

#[tokio::test]
async fn a_grant_naming_another_source_does_not_count() {
    let mut home = Home::outside_temp();
    let fake = run();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(
        &mut home,
        &granting("smart-home"),
        &[&fake],
        call,
        WORKSPACE,
        LOCAL,
        true,
    )
    .await;
    assert!(!asked(&log));
    assert!(
        fake.calls()[0].sandbox.is_some(),
        "来处对不上：照旧在沙盒里跑"
    );
}

/// 文件是链接的不算放行（施工 X-37）：照旧在沙盒里跑，照工作区这一级的规矩。
#[tokio::test]
async fn a_granted_tool_whose_file_is_a_link_stays_in_the_sandbox() {
    let mut home = Home::outside_temp();
    let fake = Fake::linked("run", Access::Execute, Act::Echo);
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(
        &mut home,
        &granting("~home"),
        &[&fake],
        call,
        WORKSPACE,
        LOCAL,
        true,
    )
    .await;
    assert!(!asked(&log));
    assert!(
        fake.calls()[0].sandbox.is_some(),
        "是链接的：照旧在沙盒里跑"
    );
}

#[tokio::test]
async fn without_a_grant_the_tool_stays_in_the_sandbox() {
    let mut home = Home::outside_temp();
    let fake = run();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(&mut home, "", &[&fake], call, WORKSPACE, LOCAL, true).await;
    assert!(!asked(&log));
    assert!(fake.calls()[0].sandbox.is_some());
}

#[tokio::test]
async fn read_only_still_refuses_a_granted_tool() {
    let mut home = Home::outside_temp();
    let fake = run();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(
        &mut home,
        &granting("~home"),
        &[&fake],
        call,
        (Level::Workspace, true),
        LOCAL,
        true,
    )
    .await;
    assert!(fake.calls().is_empty(), "只读拒，没跑");
    assert_eq!(results(&log)[0].status, ToolStatus::Denied);
}

/// 群聊（只碰得到自己工作区的会话，施工 5-12）：放行了也拒。
#[tokio::test]
async fn a_group_chat_still_refuses_a_granted_tool() {
    let mut home = Home::outside_temp();
    let fake = run();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(
        &mut home,
        &granting("~home"),
        &[&fake],
        call,
        WORKSPACE,
        ("qq:group:1", false),
        true,
    )
    .await;
    assert!(fake.calls().is_empty(), "群里放行了也拒");
    assert_eq!(results(&log)[0].status, ToolStatus::Denied);
}

/// 主人的私聊（场所会话、属主是管理员）：没人能确认，放行了的照样不问、在沙盒外跑。
#[tokio::test]
async fn the_owners_private_chat_runs_a_granted_tool() {
    let mut home = Home::outside_temp();
    let fake = run();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(
        &mut home,
        &granting("~home"),
        &[&fake],
        call,
        WORKSPACE,
        ("qq:private:10001", true),
        true,
    )
    .await;
    assert_eq!(
        results(&log)[0].status,
        ToolStatus::Ok,
        "{:?}",
        results(&log)
    );
    assert!(fake.calls()[0].sandbox.is_none());
}

/// 脚本里调到的（嵌套调用，施工 M-3）：放行了的同样不给沙盒的规格。
#[tokio::test]
async fn a_nested_call_of_a_granted_tool_runs_outside_the_sandbox() {
    let mut home = Home::outside_temp();
    let shell = Fake::new("shell", Access::Execute, Act::Calls(vec![("run", "{}")]));
    let fake = Fake::callable("run", Access::Execute, Act::Echo);
    let call = Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]);
    turn(
        &mut home,
        &granting("~home"),
        &[&shell, &fake],
        call,
        WORKSPACE,
        LOCAL,
        true,
    )
    .await;
    let calls = fake.calls();
    assert_eq!(calls.len(), 1, "嵌套调用跑了");
    assert!(calls[0].sandbox.is_none(), "放行了的不给沙盒的规格");
}

/// 这台机器上沙盒用不了（执行命令平常都问人）：放行了的照样不问。
#[tokio::test]
async fn without_a_sandbox_a_granted_tool_still_runs_without_asking() {
    let mut home = Home::outside_temp();
    let fake = run();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(
        &mut home,
        &granting("~home"),
        &[&fake],
        call,
        WORKSPACE,
        LOCAL,
        false,
    )
    .await;
    assert!(!asked(&log), "放行了的不问");
    assert_eq!(results(&log)[0].status, ToolStatus::Ok);
}

/// 工具自己报这一次要在沙盒外跑的（`shell` 的 `outside_sandbox`，施工 D-4）：放行了的不问。
#[tokio::test]
async fn a_granted_tool_asking_to_leave_the_sandbox_is_not_asked_about() {
    let mut home = Home::outside_temp();
    let fake = run();
    let call = Play::calls(&[("run", r#"{"command":"x","outside_sandbox":true}"#)]);
    let log = turn(
        &mut home,
        &granting("~home"),
        &[&fake],
        call,
        WORKSPACE,
        LOCAL,
        true,
    )
    .await;
    assert!(!asked(&log), "放行了的不问");
    assert!(fake.calls()[0].sandbox.is_none());
}
