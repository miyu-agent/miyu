//! 关起来的会话的 Unix 套接字管不住的机器上（施工 MB-1：内核的 Landlock 太老、又没有 seccomp，核心起来时探到的
//! [`miyu_sandbox::set_confines_sockets`]）：成员的、场所的会话不跑命令，回 `no-commands`，假工具一次都没跑；管理员本人的会话
//! 照跑。管得住以后照常跑。
//!
//! 只有这一个测试，自己一个进程：管不管得住是进程里一份，改了会碰到同时在跑的别的测试。

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::{Body, Level, Permission};
use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};
use support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};

/// 在 `venue` 的会话里调一次执行命令的假工具，交回它跑了几次、结果的说法。
async fn once(venue: &str, owner_is_admin: bool) -> (usize, Option<String>) {
    let home = Home::new();
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let cwd = home.scratch.0.join("work");
    std::fs::create_dir_all(&cwd).expect("建得了工作目录");
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: cwd.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        confined: !owner_is_admin,
        ..Lines::default()
    };
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let handle = home.create_full(&script, &tools, opening, lines).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let said = home
        .log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => result.human.map(|said| said.key),
            _ => None,
        });
    (run.calls().len(), said)
}

#[tokio::test]
async fn without_socket_confinement_others_run_no_command() {
    if !miyu_sandbox::CONFINES_READS {
        return;
    }
    miyu_sandbox::set_confines_sockets(false);
    for (venue, admin) in [("qq:group:1", false)] {
        let (runs, said) = once(venue, admin).await;
        assert_eq!(
            (runs, said.as_deref()),
            (0, Some("core/permissions/no-commands")),
            "{venue}"
        );
    }
    let (runs, _) = once("local", true).await;
    assert_eq!(runs, 1, "管理员本人的会话照跑");
    miyu_sandbox::set_confines_sockets(true);
    for (venue, admin) in [("qq:group:1", false)] {
        assert_eq!(once(venue, admin).await.0, 1, "{venue}：管得住以后照跑");
    }
}
