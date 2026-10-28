//! 管理员事先放行出沙盒的工具（施工 X-36，`11-权限与沙盒.md` A15，`docs/blueprint/session/guard.md`）：系统配置的
//! `sandbox.tools.<名字>` 写的来处和工具目录里这件工具归的包一样，才算放行。放行了的，工作区这一级不问、执行器不给沙盒的规格；
//! 只读拒；只碰得到自己工作区的会话（群聊）拒；来处对不上的、没写的照旧在沙盒里跑。带文件的（施工 X-37 补）：文件的真实位置
//! 落在管得住的地方（家目录自己放东西的几处、资源目录）、又不在她这一轮写得了的地方的才算。脚本里调到的（嵌套调用）走同一条链。

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
    let work = home.scratch.0.join("work");
    let level = (level, read_only);
    let venue = (venue, owner_is_admin);
    turn_in(
        home,
        &work,
        (config, tools, call),
        level,
        venue,
        (sandboxed, true),
    )
    .await
}

/// 同 [`turn`]，工作目录是 `work`、有没有人能确认照 `attended`（施工 X-37 补：资源目录就在工作区里的那两条要）。
async fn turn_in(
    home: &mut Home,
    work: &std::path::Path,
    (config, tools, call): (&str, &[&Arc<Fake>], Play),
    (level, read_only): (Level, bool),
    (venue, owner_is_admin): (&str, bool),
    (sandboxed, attended): (bool, bool),
) -> Vec<Event> {
    home.configs = configs(config, &[]);
    std::fs::create_dir_all(work).expect("建得了工作区");
    let opening = Opening {
        permission: Permission { level, read_only },
        attended,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: sandboxed.then(|| PathBuf::from(HELPER)),
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        confined: !owner_is_admin,
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

/// 放行了的、带文件的（施工 X-37 补）：文件落在管得住的地方以外的（这里是临时目录里的别处）照旧在沙盒里跑。
#[tokio::test]
async fn a_granted_tool_whose_file_is_elsewhere_stays_in_the_sandbox() {
    let mut home = Home::outside_temp();
    let file = home.scratch.0.join("elsewhere/run.py");
    std::fs::create_dir_all(file.parent().expect("有上级")).expect("建得了");
    std::fs::write(&file, "x").expect("写得进");
    let fake = Fake::with_file("run", Access::Execute, file, Act::Echo);
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
        "不在管得住的地方：照旧在沙盒里跑"
    );
}

/// 文件在家目录的 `scripts/` 里的照常放行：不问、不给沙盒的规格。
#[tokio::test]
async fn a_granted_script_in_the_home_folder_runs_outside() {
    let mut home = Home::outside_temp();
    let file = home
        .root
        .account_dir(&alice_account())
        .join("scripts/run.py");
    std::fs::create_dir_all(file.parent().expect("有上级")).expect("建得了");
    std::fs::write(&file, "x").expect("写得进");
    let fake = Fake::with_file("run", Access::Execute, file, Act::Echo);
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
        fake.calls()[0].sandbox.is_none(),
        "家目录里的：不给沙盒的规格"
    );
}

/// 纵深防御（施工 X-37 补，主会话审时加）：开发时资源目录是仓库检出，她的工作区就是它的时候，落在资源目录里的照旧在沙盒里跑；
/// 工作区换到别处，同一件就放行。
#[tokio::test]
async fn a_granted_file_in_her_workspace_stays_in_the_sandbox() {
    let resources = std::fs::canonicalize(home_resources()).expect("在");
    let file = resources.join("core/mcp/cut.txt");
    let fake = Fake::with_file("run", Access::Execute, file.clone(), Act::Echo);
    let mut home = Home::outside_temp();
    let call = Play::calls(&[("run", "{}")]);
    let config = granting("~home");
    turn_in(
        &mut home,
        &resources,
        (&config, &[&fake], call),
        WORKSPACE,
        LOCAL,
        (true, true),
    )
    .await;
    assert!(
        fake.calls()[0].sandbox.is_some(),
        "资源目录就是工作区：照旧在沙盒里跑"
    );
    let elsewhere = Fake::with_file("run", Access::Execute, file, Act::Echo);
    let mut home = Home::outside_temp();
    let call = Play::calls(&[("run", "{}")]);
    turn(
        &mut home,
        &config,
        &[&elsewhere],
        call,
        WORKSPACE,
        LOCAL,
        true,
    )
    .await;
    assert!(
        elsewhere.calls()[0].sandbox.is_none(),
        "工作区在别处：资源目录里的放行"
    );
}

/// 同上，这台机器的沙盒用不了时（施工 X-37 补）：执行器本来就不套，权限策略照「写得了的地方不算」判：她的工作区里的不算放行，
/// 照没放行的问人，这里没人能确认，拒；工作区在别处的放行、照常跑。
#[tokio::test]
async fn without_a_sandbox_a_granted_file_in_her_workspace_is_not_granted() {
    let resources = std::fs::canonicalize(home_resources()).expect("在");
    let file = resources.join("core/mcp/cut.txt");
    let config = granting("~home");
    let fake = Fake::with_file("run", Access::Execute, file.clone(), Act::Echo);
    let mut home = Home::outside_temp();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn_in(
        &mut home,
        &resources,
        (&config, &[&fake], call),
        WORKSPACE,
        LOCAL,
        (false, false),
    )
    .await;
    assert_eq!(
        results(&log)[0].status,
        ToolStatus::Denied,
        "工作区里的：不算放行，没人能确认就拒"
    );
    assert!(fake.calls().is_empty(), "没跑");
    let elsewhere = Fake::with_file("run", Access::Execute, file, Act::Echo);
    let mut home = Home::outside_temp();
    let call = Play::calls(&[("run", "{}")]);
    let log = turn(
        &mut home,
        &config,
        &[&elsewhere],
        call,
        WORKSPACE,
        LOCAL,
        false,
    )
    .await;
    assert!(!asked(&log), "工作区在别处：放行，不问");
    assert_eq!(results(&log)[0].status, ToolStatus::Ok);
}

/// 测试用的核心资源目录：仓库的 `resources/`（同 `Home` 用的那一份）。
fn home_resources() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
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
    let shell = Fake::commanding(
        "shell",
        Access::Execute,
        Some(miyu_tool::RUN_IN_BACKGROUND),
        Act::Calls(vec![("run", "{}")]),
    );
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
