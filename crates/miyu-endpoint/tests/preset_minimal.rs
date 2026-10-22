//! 出厂的极简开发（施工 P-16 上，`docs/blueprint/presets.md`「出厂的几份」）：只开基础系统的「文件读写」「运行命令」两个功能，
//! 再单独关掉 glob、grep、trash；基础系统别的功能、别的包都照 `unlisted = "off"` 关着。真核心走一遍：发出去的工具正好四件。

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
    Catalog::in_packages([
        ("basesystem", basesystem.map(fake).to_vec()),
        ("memory", vec![fake("remember")]),
    ])
    .expect("合写法")
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
