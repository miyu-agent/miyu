//! 沙盒藏起别的程序的私密目录（施工 5-13，`docs/blueprint/sandbox.md` 的 `hidden`）：这台机器上有的才写，排在数据根后面，
//! 照清单的先后。A16 起本人的会话只读时才进沙盒（施工 LV-1），哪儿都不能写，也就没有落在能写的里面不藏的（原来工作区一级的
//! 那几条随它删了）。Unix 上有收紧手段的，真的经助手跑 `shell`：私密目录里的读不到，家目录别处照读。场地的假家目录是 `home/`。

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

/// 在 `cwd` 里造一个只读的会话，沙盒能用（假的助手，假工具不起它）：执行命令的假工具 `run` 跑一次，交回它拿到的沙盒。
async fn attached(home: &Home, cwd: &Path) -> Arc<Sandboxed> {
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: true,
        },
        attended: false,
        cwd: cwd.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
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
    let sandboxed = attached(&home, &work).await;
    assert_eq!(
        sandboxed.spec.hidden, expected,
        "有的才写，照清单的先后，没有的（.gnupg）不写"
    );
}

/// 真的经助手跑 `shell`（Unix 上有收紧手段的，只读）：私密目录里的读不到，家目录别处照读。
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
            level: Level::Full,
            read_only: true,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(helper),
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
