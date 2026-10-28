//! 会话的附件目录（施工 3-9 六补，`docs/blueprint/fs.md` 第一节第 2 行）：落成文件的附件放在会话目录的 `attachments/` 里，
//! 这个会话读得到、写不了；别的会话的附件目录照数据根，谁都不能碰。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, ToolStatus};
use miyu_kernel::id::SessionId;
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 会话 `session` 的附件目录里放一份 `name`，交回她照抄的路径（Windows 上反斜杠换成 `/`）。
fn put(home: &Home, session: &str, name: &str) -> String {
    let dir =
        miyu_store::attachments::dir(&home.root, &SessionId::parse(session).expect("会话编号"));
    let path = miyu_store::attachments::keep(&dir, name, "text/plain", b"x").expect("存得下");
    let path = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path
    }
}

fn one(tool: &str, path: &str) -> Play {
    Play::calls(&[(tool, &serde_json::json!({ "path": path }).to_string())])
}

/// 工作区在 `work/` 的本机会话，工具面是读、写两件假工具。
async fn session(home: &Home, script: &Script, catalog: &Catalog) -> Handle {
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作区");
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    home.create_full(script, catalog, opening, Lines::default())
        .await
}

/// 说一句，等这一轮说完；交回日志。
async fn turn(home: &Home, handle: &Handle, command: &str) -> Vec<Event> {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    home.log(handle.id())
}

fn statuses(log: &[Event]) -> Vec<ToolStatus> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.status.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn her_own_attachments_are_readable_and_nobody_elses() {
    let home = Home::new();
    let read = Fake::new("read", Access::Read, Act::Echo);
    let write = Fake::new("write", Access::Write, Act::Echo);
    let catalog = Catalog::new([
        Arc::clone(&read) as Arc<dyn Tool>,
        Arc::clone(&write) as Arc<dyn Tool>,
    ])
    .expect("合写法");
    // 先开两个会话，拿到编号，各自的附件目录里放一份。
    let other = session(&home, &Script::new([Play::Says("嗯。")]), &catalog).await;
    let script = Script::new([Play::Says("嗯。")]);
    let handle = session(&home, &script, &catalog).await;
    turn(&home, &handle, "cmd-1").await;
    stop(&handle).await;
    let mine = put(&home, handle.id().as_str(), "截图.png");
    let theirs = put(&home, other.id().as_str(), "别人的.png");
    assert!(Path::new(&mine).is_file());
    // 载入以后（附件是收到话时才落的，这一刻目录已经在）：读自己的放行，写自己的拒，读别人的拒。
    let again = Script::new([
        one("read", &mine),
        one("write", &mine),
        one("read", &theirs),
        Play::Says("好。"),
    ]);
    let loaded = home.load_with(handle.id(), &again, &catalog).await;
    let log = turn(&home, &loaded, "cmd-2").await;
    assert_eq!(read.calls().len(), 1, "读自己的附件放行：{log:#?}");
    assert!(write.calls().is_empty(), "写一律拒（完全放开也拒）");
    assert_eq!(
        statuses(&log)[statuses(&log).len() - 3..],
        [ToolStatus::Ok, ToolStatus::Denied, ToolStatus::Denied],
        "别的会话的附件照数据根，不给碰"
    );
}
