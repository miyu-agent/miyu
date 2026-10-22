//! 沙盒藏起别的程序的私密目录（施工 5-13，`docs/blueprint/sandbox.md` 的 `hidden`）：这台机器上有的才写，排在数据根后面，
//! 照清单的先后；落在能写的里面的不藏（工作区装着它、加进来的目录装着它；工作目录太宽（根目录）的，能写的退回会话的
//! 默认工作区，施工 5-14，照藏；家目录施工 5-14 改起不再太宽，macOS 上照藏、Linux 上有挂载才藏）。Unix 上有收紧手段的，真的经助手跑
//! `shell`：私密目录里的读不到，家目录别处照读。场地的假家目录是 `home/`。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::tool::Access;
use miyu_sandbox::Sandboxed;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 真实的位置。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("在")
}

/// 在 `cwd` 里造一个会话（工作区这一级，`read_only` 时只读，加进来的目录 `dirs`），沙盒能用（假的助手，假工具不起它）：
/// 执行命令的假工具 `run` 跑一次，交回它拿到的沙盒。
async fn attached(home: &Home, cwd: &Path, read_only: bool, dirs: Vec<String>) -> Arc<Sandboxed> {
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only,
        },
        attended: false,
        cwd: cwd.to_string_lossy().into_owned(),
        dirs,
        sandbox: Some(PathBuf::from("miyu-sandbox")),
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let calls = run.calls();
    assert_eq!(calls.len(), 1, "跑了一次");
    on_site(home, calls[0].sandbox.clone().expect("进了沙盒"))
}

#[tokio::test]
async fn the_private_dirs_on_this_machine_are_hidden_after_the_data_root() {
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    for dir in [".claude", ".ssh", ".config/gh"] {
        std::fs::create_dir_all(home.home.join(dir)).expect("建得了目录");
    }
    let expected = vec![
        real(home.root.path()),
        real(&home.home.join(".ssh")),
        real(&home.home.join(".claude")),
        real(&home.home.join(".config/gh")),
    ];
    for read_only in [false, true] {
        let sandboxed = attached(&home, &work, read_only, Vec::new()).await;
        assert_eq!(
            sandboxed.spec.hidden, expected,
            "只读 {read_only}：有的才写，照清单的先后，没有的（.gnupg）不写"
        );
    }
}

#[tokio::test]
async fn a_private_dir_inside_a_writable_place_is_not_hidden() {
    let home = Home::outside_temp();
    for dir in [".ssh", ".claude"] {
        std::fs::create_dir_all(home.home.join(dir)).expect("建得了目录");
    }
    // 工作目录太宽（根目录）：能写的是这个会话的默认工作区（施工 5-14），私密目录不在里面，照藏。
    let root = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    let sandboxed = attached(&home, &root, false, Vec::new()).await;
    let ssh_claude = [
        real(home.root.path()),
        real(&home.home.join(".ssh")),
        real(&home.home.join(".claude")),
    ];
    assert_eq!(sandboxed.spec.hidden, ssh_claude, "根目录太宽：照藏");
    let workspace = home.root.workspace(&alice_account());
    assert!(
        sandboxed.spec.write[0].starts_with(real(&workspace)),
        "能写的是会话的默认工作区：{:?}",
        sandboxed.spec.write
    );
    // 工作目录是家目录（施工 5-14 改：Linux、macOS 上不再太宽，家目录就是工作区）：落在里面的私密目录 macOS 上照藏（Seatbelt
    // 藏得住），Linux 上没有挂载的不藏（这个测试的进程里没设挂载）。Windows 上照旧退回，照藏。
    let sandboxed = attached(&home, &home.home, false, Vec::new()).await;
    if cfg!(windows) || cfg!(target_os = "macos") {
        assert_eq!(sandboxed.spec.hidden, ssh_claude, "家目录：照藏");
    } else {
        assert_eq!(
            sandboxed.spec.hidden,
            [real(home.root.path())],
            "家目录就是工作区，没有挂载：落在里面的私密目录不藏"
        );
        assert_eq!(sandboxed.spec.write[0], real(&home.home));
    }
    // 工作区开在 `~/.config`：里面的 `gh` 藏得住的照藏（macOS，最深的那一片算，施工 5-14 改），Linux 没有挂载的不藏。
    std::fs::create_dir_all(home.home.join(".config/gh")).expect("建得了目录");
    let sandboxed = attached(&home, &home.home.join(".config"), false, Vec::new()).await;
    let gh = real(&home.home.join(".config/gh"));
    let mut config = ssh_claude.to_vec();
    if cfg!(target_os = "macos") {
        config.push(gh.clone());
    }
    assert_eq!(
        sandboxed.spec.hidden, config,
        "工作区装着它：藏得住的照藏，藏不住的照人给的算"
    );
    let work = home.scratch.0.join("work");
    let claude = home.home.join(".claude").to_string_lossy().into_owned();
    let sandboxed = attached(&home, &work, false, vec![claude]).await;
    assert_eq!(
        sandboxed.spec.hidden,
        [real(home.root.path()), real(&home.home.join(".ssh")), gh],
        "加进来的目录就是它：不藏（macOS 上也一样，人给的优先）"
    );
}

/// 真的经助手跑 `shell`（Unix 上有收紧手段的）：私密目录里的读不到，家目录别处照读。
#[cfg(unix)]
#[tokio::test]
async fn a_sandboxed_command_cannot_read_a_private_dir() {
    use miyu_kernel::event::{Body, ToolStatus};

    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, std::time::Duration::from_secs(60)).expect("探得了");
    if probe.mechanisms.is_empty() {
        return;
    }
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(home.home.join(".ssh")).expect("建得了目录");
    std::fs::write(home.home.join(".ssh/id_ed25519"), "key\n").expect("写得进");
    std::fs::write(home.home.join("notes.txt"), "notes\n").expect("写得进");
    let command = format!(
        "cat '{}'; cat '{}' 2>/dev/null || echo blocked-private",
        home.home.join("notes.txt").display(),
        home.home.join(".ssh/id_ed25519").display()
    );
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let script = Script::new([Play::calls(&[("shell", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(helper),
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let result = home
        .log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一条工具结果");
    assert_eq!(result.status, ToolStatus::Ok, "{result:?}");
    let text: String = result
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect();
    assert_eq!(text, "notes\nblocked-private\n");
}
