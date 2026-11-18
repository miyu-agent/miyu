//! 沙盒写不成的那一行运行日志（施工 5-4 上）：关起来的会话（群）工作区只读，里面的 `.tmp` 建不成，这次调用不跑，
//! 记一行 `ERROR` 写明原因，再照崩了记一行。施工 RO-2 起本人只读的规格什么都不藏、不碰磁盘，写不成的只剩关起来的会话（原来
//! 占的是只读时命令起来以前要补上的数据根里的一处，施工 R3-1）。关不住读的平台上关起来的会话不跑命令，这里到此为止。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，
//! 这里装的订阅者可能漏听。

// 工作区改只读用的是 Unix 的权限位；关不住读的 Windows 上关起来的会话本来就不跑命令。
#![cfg(unix)]

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};
use support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};

#[tokio::test]
async fn a_sandbox_that_cannot_be_set_up_is_logged_with_the_reason() {
    if !miyu_sandbox::confines_others() {
        return;
    }
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    // 工作区只读、里面的 `.tmp` 建不成（施工 AU-001 起占着位置的文件、链接会被删掉重建，换成建不了）。原因照在那里建目录报的
    // 错写。
    let home = Home::new();
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作区");
    let shut = |mode| {
        std::fs::set_permissions(&work, std::os::unix::fs::PermissionsExt::from_mode(mode))
            .expect("改得了权限");
    };
    shut(0o555);
    let Err(reason) = std::fs::create_dir(work.join(".tmp")) else {
        // 照样建得成（比如跑测试的是 root）：造不出这种情况。
        shut(0o755);
        return;
    };
    let reason = reason.to_string();
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
    };
    let lines = Lines {
        venue: VenueId::parse("qq:group:1").expect("场所合写法"),
        confined: true,
        ..Lines::default()
    };
    let handle = home.create_full(&script, &tools, opening, lines).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    shut(0o755);
    assert!(run.calls().is_empty(), "没跑");

    let lines: Vec<String> = memory
        .lines()
        .into_iter()
        .filter(|line| line.contains(handle.id().as_str()) && line.contains(" call="))
        .collect();
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert!(lines[0].contains(" INFO ") && lines[0].contains(" running call="));
    assert!(
        lines[1].contains(" ERROR ")
            && lines[1].contains(" sandbox not set up call=")
            && lines[1].ends_with(&format!(" error={reason:?}")),
        "{lines:#?}"
    );
    assert!(
        lines[2].contains(" ERROR ") && lines[2].contains(" crashed call="),
        "{lines:#?}"
    );
}
