//! 技能（施工 X-10 上，`docs/blueprint/skills.md`「技能」第 2、3 条）：属主家目录里放的技能列进 system，一个一行带 `SKILL.md`
//! 的路径；技能的文件夹在数据根里，单独放开只读：读放行、写一律拒（完全放开也拒），旁边的数据根照旧谁都不能碰；只碰得到自己
//! 工作区的群会话也读得到；载入以后照样放开。工具面上没有 `read` 的不列。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, ToolStatus};
use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 读、写两件假工具，报参数里的路径。
struct Kit {
    read: Arc<Fake>,
    write: Arc<Fake>,
}

impl Kit {
    fn new() -> Kit {
        Kit {
            read: Fake::new("read", Access::Read, Act::Echo),
            write: Fake::new("write", Access::Write, Act::Echo),
        }
    }

    fn catalog(&self) -> Catalog {
        Catalog::new([
            Arc::clone(&self.read) as Arc<dyn Tool>,
            Arc::clone(&self.write) as Arc<dyn Tool>,
        ])
        .expect("合写法")
    }
}

/// alice 家目录里的一处。
fn mine(home: &Home, path: &str) -> PathBuf {
    home.root.account_dir(&alice_account()).join(path)
}

/// 在 alice 家目录里放技能 `notes`、旁边放一份不给碰的；交回 `SKILL.md` 列出来的路径（Windows 上反斜杠换成了 `/`，她照抄
/// 这一条读）。
fn put(home: &Home) -> String {
    let file = mine(home, "skills").join("notes").join("SKILL.md");
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(
        &file,
        "---\nname: notes\ndescription: Take notes.\n---\n\n照这样记。\n",
    )
    .expect("写得进");
    std::fs::write(mine(home, "secret.txt"), "不给看").expect("写得进");
    let file = file.to_string_lossy().into_owned();
    if cfg!(windows) {
        file.replace('\\', "/")
    } else {
        file
    }
}

fn one(tool: &str, path: &str) -> Play {
    Play::calls(&[(tool, &serde_json::json!({ "path": path }).to_string())])
}

/// 工作区在 `work/` 的会话，这一级是 `level`，场所 `venue`，属主是不是管理员照 `owner_is_admin`。
async fn session(
    home: &Home,
    script: &Script,
    kit: &Kit,
    level: Level,
    (venue, owner_is_admin): (&str, bool),
) -> Handle {
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作区");
    let opening = Opening {
        permission: Permission {
            level,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        owner_is_admin,
        ..Lines::default()
    };
    home.create_full(script, &kit.catalog(), opening, lines)
        .await
}

/// 说一句，等这一轮说完；交回日志。
async fn turn(home: &Home, handle: &Handle, command: &str) -> Vec<Event> {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    home.log(handle.id())
}

/// 日志里工具结果的状态，照先后。
fn statuses(log: &[Event]) -> Vec<ToolStatus> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.status.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn her_skills_are_listed_and_their_folder_is_read_only() {
    for level in [Level::Workspace, Level::Full] {
        let home = Home::new();
        let kit = Kit::new();
        let skill = put(&home);
        let secret = mine(&home, "secret.txt").to_string_lossy().into_owned();
        let script = Script::new([
            one("read", &skill),
            one("write", &skill),
            one("read", &secret),
            Play::Says("好。"),
        ]);
        let handle = session(&home, &script, &kit, level.clone(), ("local", true)).await;
        let log = turn(&home, &handle, "cmd-1").await;
        assert_eq!(kit.read.calls().len(), 1, "{level:?}：读技能放行");
        assert!(kit.write.calls().is_empty(), "{level:?}：写技能一律拒");
        assert_eq!(
            statuses(&log),
            [ToolStatus::Ok, ToolStatus::Denied, ToolStatus::Denied],
            "{level:?}：旁边的数据根照旧不给碰"
        );
        let system = &script.requests()[0].1.system;
        assert!(
            system.ends_with(&format!(
                "Skills you can use (read the file before following one):\n- notes: Take notes. ({skill})"
            )),
            "列在 system 末尾：{system}"
        );
    }
}

#[tokio::test]
async fn a_confined_group_session_reads_skills_too() {
    let home = Home::new();
    let kit = Kit::new();
    let skill = put(&home);
    let script = Script::new([
        one("read", &skill),
        one("write", &skill),
        Play::Says("好。"),
    ]);
    let handle = session(
        &home,
        &script,
        &kit,
        Level::Workspace,
        ("qq:group:1", false),
    )
    .await;
    let log = turn(&home, &handle, "cmd-1").await;
    assert_eq!(kit.read.calls().len(), 1, "读放行：{log:#?}");
    assert_eq!(statuses(&log), [ToolStatus::Ok, ToolStatus::Denied]);
}

#[tokio::test]
async fn a_loaded_session_keeps_its_list_and_still_reads_skills() {
    let home = Home::new();
    let kit = Kit::new();
    let skill = put(&home);
    let script = Script::new([Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Workspace, ("local", true)).await;
    turn(&home, &handle, "cmd-1").await;
    stop(&handle).await;
    let again = Script::new([one("read", &skill), Play::Says("嗯。")]);
    let loaded = home.load_with(handle.id(), &again, &kit.catalog()).await;
    let log = turn(&home, &loaded, "cmd-2").await;
    assert_eq!(kit.read.calls().len(), 1, "载入以后照样放开");
    assert_eq!(statuses(&log), [ToolStatus::Ok]);
    assert_eq!(
        again.requests()[0].1.system,
        script.requests()[0].1.system,
        "列出来的那一块照快照"
    );
}
