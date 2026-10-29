//! 预设和记忆（施工 P-15，2026-10-20 项目主人定：出厂的常规开发也开记忆，从 `preset_face.rs` 挪出来）：没开记忆的预设
//! 工具面上没有记忆的三件，system 末尾说装了没开，快照的范围是 `off`，开会话时要了也没用；功能关着的，它的工具在
//! `preset.get` 里都关着；出厂的常规开发开着。

use serde_json::json;

use crate::preset_face::{
    configured, connected, feature, first_request, memory_of, snapshot, tool_names,
};
use crate::support::*;
use miyu_session::testkit::Script;

/// 和常规开发一样、只是不开记忆（出厂的两个都开着，施工 P-15）。
const QUIET: &str = "[preset]\nname = \"安静\"\nunlisted = \"off\"\n\n[software]\nbasesystem = true\nmiyu-base-net = true\nmiyu-base-goal = true\nmiyu-base-authoring = true\n";

#[tokio::test]
async fn a_preset_without_memory_drops_it() {
    let home = Home::new();
    home.write("presets/quiet.toml", QUIET);
    let (session, request) = first_request(&home, json!({"cwd": "~", "preset": "quiet"})).await;
    assert_eq!(tool_names(&request), ["read", "shell"], "记忆没开");
    assert!(
        !request.system.contains("Installed but off"),
        "没开的那一行不写（施工 P-OFF）：{}",
        request.system
    );
    let snapshot = snapshot(&home, &session);
    assert_eq!(
        snapshot.memory.as_deref(),
        Some("off"),
        "记忆没开的范围是 off"
    );
    let pin = snapshot.preset.expect("记了预设");
    assert_eq!(pin.id, "quiet");
    // 记的是功能的编号（施工 F-3 上），装了没开的照记，见下一条；知识库（施工 K-1 下）在它里面关着；环境那几块（施工 ENV-1 起在
    // env 包里）这个预设没开。
    assert_eq!(
        pin.off,
        [
            "env-harness",
            "env-machine",
            "env-model",
            "env-permission",
            "env-profile",
            "env-session",
            "env-time",
            "env-workdir",
            "knowledge",
            "memory",
            "roleplay"
        ]
    );
    // 开会话时要了记忆也没用：开不开归预设。
    let (session, _) = first_request(
        &home,
        json!({"cwd": "~", "preset": "quiet", "memory": "session"}),
    )
    .await;
    assert_eq!(memory_of(&home, &session).as_deref(), Some("off"));
    // 出厂的常规开发开着记忆（施工 P-15，2026-10-20 项目主人定）。
    let (session, request) = first_request(&home, json!({"cwd": "~", "preset": "dev"})).await;
    let names = ["forget", "memory_search", "read", "remember", "shell"];
    assert_eq!(tool_names(&request), names);
    assert_eq!(memory_of(&home, &session).as_deref(), Some("persona"));
}

#[tokio::test]
async fn a_feature_that_is_off_has_all_its_tools_off() {
    let home = Home::new();
    home.write("presets/quiet.toml", QUIET);
    let mut client = connected(configured(&home, &Script::new([]))).await;
    let quiet = client
        .call("g1", "preset.get", json!({"preset": "quiet"}))
        .await;
    assert_eq!(
        feature(&quiet, "memory")["tools"],
        json!([
            {"name": "forget", "label": "忘掉", "on": false},
            {"name": "memory_search", "label": "回忆", "on": false},
            {"name": "remember", "label": "记住", "on": false},
        ]),
        "功能关着的工具都是关着"
    );
}
