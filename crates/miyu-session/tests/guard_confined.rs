//! 只碰得到自己工作区的会话（施工 5-12，`11-权限与沙盒.md` 第三节「外部身份」）：关起来的场所会话，文件类工具只放行
//! 工作区里的，外面的（连读）一律拒，写给她的是出厂的那一句，说法 `core/permissions/outside-workspace`；命令一律拒（沙盒把读
//! 也关进工作区以后再放开）。没关起来的场所会话、本机的会话照旧。场地不在
//! 系统的临时目录里：工作区 `work/`。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, ToolResult, ToolStatus};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, Workplace};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 读、写、跑命令的三件假工具，报参数里的路径。
struct Kit {
    read: Arc<Fake>,
    write: Arc<Fake>,
    shell: Arc<Fake>,
}

impl Kit {
    fn new() -> Kit {
        Kit {
            read: Fake::new("read", Access::Read, Act::Echo),
            write: Fake::new("write", Access::Write, Act::Echo),
            shell: Fake::commanding(
                "shell",
                Access::Execute,
                Some(miyu_tool::RUN_IN_BACKGROUND),
                Act::Echo,
            ),
        }
    }

    fn catalog(&self) -> Catalog {
        Catalog::new([
            Arc::clone(&self.read) as Arc<dyn Tool>,
            Arc::clone(&self.write) as Arc<dyn Tool>,
            Arc::clone(&self.shell) as Arc<dyn Tool>,
        ])
        .expect("合写法")
    }
}

/// 场地里的一处，写成绝对路径。
fn at(home: &Home, path: &str) -> String {
    home.scratch.0.join(path).to_string_lossy().into_owned()
}

/// 调一件工具，参数里是一条路径。
fn one(tool: &str, path: &str) -> Play {
    Play::calls(&[(tool, &serde_json::json!({ "path": path }).to_string())])
}

/// 场所 `venue` 里、工作区 `work/` 的会话：工作区这一级，没人能确认，`owner_is_admin` 是假的关起来。
async fn session(
    home: &Home,
    script: &Script,
    kit: &Kit,
    venue: &str,
    owner_is_admin: bool,
) -> Handle {
    std::fs::create_dir_all(home.scratch.0.join("work")).expect("建得了工作区");
    std::fs::create_dir_all(home.scratch.0.join("elsewhere")).expect("建得了别处");
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: at(home, "work"),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        confined: !owner_is_admin,
        ..Lines::default()
    };
    home.create_full(script, &kit.catalog(), opening, lines)
        .await
}

/// 说一句，等这一轮说完；交回日志。
async fn turn(home: &Home, handle: &Handle) -> Vec<Event> {
    let mut pushes = watch(handle).await;
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    home.log(handle.id())
}

/// 日志里的工具结果，照先后。
fn results(log: &[Event]) -> Vec<ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.clone()),
            _ => None,
        })
        .collect()
}

/// 一条工具结果的字。
fn text(result: &ToolResult) -> String {
    match result.blocks.as_slice() {
        [miyu_kernel::block::Block::Text(text)] => text.text.clone(),
        other => panic!("{other:?}"),
    }
}

fn shipped(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/core/permissions")
            .join(name),
    )
    .expect("出厂有")
}

#[tokio::test]
async fn a_group_session_only_touches_its_workspace_and_runs_no_command() {
    let home = Home::new();
    let kit = Kit::new();
    let inside = at(&home, "work/notes.md");
    let outside = at(&home, "elsewhere/secret.txt");
    let script = Script::new([
        one("read", &inside),
        one("write", &inside),
        one("read", &outside),
        one("write", &outside),
        Play::calls(&[("shell", "{\"command\": \"ls\"}")]),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, &kit, "qq:group:1", false).await;
    let log = turn(&home, &handle).await;
    assert_eq!(kit.read.calls().len(), 1, "工作区里的读放行：{log:#?}");
    assert_eq!(kit.write.calls().len(), 1, "工作区里的写放行");
    assert!(kit.shell.calls().is_empty(), "命令一律拒");
    let statuses: Vec<ToolStatus> = results(&log)
        .iter()
        .map(|result| result.status.clone())
        .collect();
    assert_eq!(
        statuses,
        [
            ToolStatus::Ok,
            ToolStatus::Ok,
            ToolStatus::Denied,
            ToolStatus::Denied,
            ToolStatus::Denied
        ]
    );
    let all = results(&log);
    assert_eq!(
        text(&all[2]),
        miyu_kernel::template::Template::parse(&shipped("outside-workspace.txt"))
            .expect("出厂的写法对")
            .render(&std::collections::BTreeMap::from([(
                "path",
                outside.as_str()
            )]))
            .expect("换得进"),
        "照出厂的那一句"
    );
    assert_eq!(
        all[2].human.as_ref().map(|said| said.key.as_str()),
        Some("core/permissions/outside-workspace")
    );
    assert_eq!(text(&all[4]), shipped("no-commands.txt"));
}

#[tokio::test]
async fn the_admins_own_chat_and_local_sessions_are_not_confined() {
    for venue in ["qq:private:10001", "local"] {
        let home = Home::new();
        let kit = Kit::new();
        let outside = at(&home, "elsewhere/notes.md");
        let script = Script::new([one("read", &outside), Play::Says("好。")]);
        let handle = session(&home, &script, &kit, venue, true).await;
        turn(&home, &handle).await;
        assert_eq!(kit.read.calls().len(), 1, "{venue}：边界以外的读照旧放行");
    }
}

/// 关起来的会话载入以后照样关着（施工 Y-2 下上）：照日志第一条记的 `confined`，载入的一方交的是「不关」也一样（施工 SU-4 中）。
#[tokio::test]
async fn a_confined_session_stays_confined_after_loading() {
    let home = Home::new();
    let kit = Kit::new();
    let outside = at(&home, "elsewhere/secret.txt");
    let script = Script::new([
        Play::Says("好。"),
        one("read", &outside),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, &kit, "local", false).await;
    turn(&home, &handle).await;
    stop(&handle).await;
    let loaded = home
        .load_placed(
            handle.id(),
            &script,
            &kit.catalog(),
            Workplace::Given(Environment {
                cwd: at(&home, "work"),
                ..environment()
            }),
            None,
        )
        .await;
    let mut pushes = watch(&loaded).await;
    ask(&loaded, "cmd-2", say("再来")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert!(kit.read.calls().is_empty(), "载入以后边界以外的读照样拒");
}

/// 关起来的会话分叉出来的照旧关着（施工 SU-4 中）：桥分叉群会话，分支记下 `confined`，载入时照它关。
#[tokio::test]
async fn a_branch_of_a_confined_session_is_confined_too() {
    let home = Home::new();
    let kit = Kit::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = session(&home, &script, &kit, "qq:group:1", false).await;
    turn(&home, &handle).await;
    let child = miyu_session::new_id(now());
    miyu_session::fork(miyu_session::Fork {
        root: &home.root,
        parent: handle.id(),
        id: child.clone(),
        cause: miyu_kernel::id::CommandId::parse("f1").expect("合写法"),
        by: alice(),
        at: now(),
        index: None,
        seal_after: None,
        within: None,
    })
    .await
    .expect("分叉得了");
    let first = home.log(&child).into_iter().next().expect("有第一条");
    let Body::SessionCreated(created) = first.body else {
        panic!("第一条是 session.created：{first:?}");
    };
    assert!(created.confined);
}

/// 关起来的会话装不了包（施工 SU-4 中：照关不关在闸上拒）：完全放开
/// 的也一样，回出厂的那一句。不关的会话完全放开照常放行。
#[tokio::test]
async fn a_confined_session_installs_no_package() {
    for (venue, confined) in [("qq:group:1", true), ("local", false)] {
        let home = Home::new();
        let package = Fake::new("package", Access::Install, Act::Echo);
        let catalog = Catalog::new([Arc::clone(&package) as Arc<dyn Tool>]).expect("合写法");
        let script = Script::new([one("package", "/x"), Play::Says("好。")]);
        let opening = Opening {
            permission: Permission {
                level: Level::Full,
                read_only: false,
            },
            attended: false,
            cwd: at(&home, "work"),
            dirs: Vec::new(),
            sandbox: None,
            sandbox_cache: None,
        };
        std::fs::create_dir_all(home.scratch.0.join("work")).expect("建得了工作区");
        let lines = Lines {
            venue: VenueId::parse(venue).expect("场所合写法"),
            confined,
            ..Lines::default()
        };
        let handle = home.create_full(&script, &catalog, opening, lines).await;
        let log = turn(&home, &handle).await;
        match confined {
            true => {
                assert!(package.calls().is_empty(), "关着的拒");
                assert_eq!(text(&results(&log)[0]), shipped("no-commands.txt"));
            }
            false => assert_eq!(package.calls().len(), 1, "不关的完全放开照常装"),
        }
    }
}
