//! 出厂的极简开发（施工 P-16 上，`docs/blueprint/presets.md`「出厂的几份」）：只开基础系统的「文件读写」「运行命令」两个功能，
//! 再单独关掉 glob、grep、trash；基础系统别的功能、别的包都照 `unlisted = "off"` 关着（项目指令也关着，施工 P-14）。真核心
//! 走一遍：发出去的工具正好四件。

use std::sync::Arc;

use serde_json::json;

use miyu_endpoint::Core;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::preset_face::{connected, tool_names};
use crate::support::venues::configured_core;
use crate::support::*;

/// 基础系统现在的全部工具（照它的清单），加记忆的一件：都照包登记。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    let basesystem = [
        "read",
        "write",
        "edit",
        "glob",
        "grep",
        "trash",
        "shell",
        "jobs",
        "subagent",
        "sessions",
        "send_message",
        "ask_user",
        "todowrite",
        "history",
        "session_usage",
        "packages",
        "package",
        "show",
    ];
    // 施工 BS-1 上中起一件一个包（`miyu_basesystem::grouped`）。
    let mut groups = miyu_basesystem::grouped(basesystem.map(fake).to_vec());
    groups.push(("memory", vec![fake("remember")]));
    Catalog::in_packages(groups).expect("合写法")
}

#[tokio::test]
async fn the_minimal_preset_gives_only_the_shell_and_the_file_tools() {
    let home = Home::new();
    let script = Script::new([Play::Says("嗯。")]);
    let core: Arc<Core> = configured_core(&home, &script, catalog());
    let mut client = connected(core).await;
    let reply = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": "~", "preset": "minimal"}),
        )
        .await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    assert_eq!(tool_names(&request), ["edit", "read", "shell", "write"]);
}

/// 项目指令（施工 P-14，2026-10-23 项目主人定）：极简开发不读仓库里的 AGENTS.md，常规开发读。
#[tokio::test]
async fn the_minimal_preset_leaves_project_instructions_out() {
    let home = Home::new();
    let repo = home.work.join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect("建得了");
    std::fs::write(repo.join("AGENTS.md"), "House rules.\n").expect("写得进");
    let script = Script::new([Play::Says("嗯。"), Play::Says("嗯。")]);
    let core: Arc<Core> = configured_core(&home, &script, catalog());
    let mut client = connected(core).await;
    for (n, preset) in ["minimal", "dev"].into_iter().enumerate() {
        let reply = client
            .call(
                &format!("c{n}"),
                "session.create",
                json!({"cwd": repo.to_string_lossy(), "preset": preset}),
            )
            .await;
        let session = reply["result"]["session"]
            .as_str()
            .unwrap_or_else(|| panic!("没造出来：{reply}"))
            .to_string();
        client.say(&format!("s{n}"), &session, "hi").await;
        home.until_turns(&session, 1).await;
    }
    let requests = script.requests();
    let told = |n: usize| format!("{:?}", requests[n].1).contains("House rules.");
    assert!(!told(0), "极简开发不读");
    assert!(told(1), "常规开发读");
}
