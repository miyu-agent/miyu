//! `preset.get` 的 `sections`（施工 FG-1，设计 52 第一节第 3 条、第三节）：带几个功能的扩展自己一组、只带一个的合进「扩展」，排在
//! 出厂组后面、技能前面；没有单功能扩展的不列「扩展」；重名的带包名；名字照连接的语言。共用的在 `preset_sections.rs`。

use serde_json::json;

use crate::preset_sections::{connected, get, heads, item, rows, section};
use crate::support::extensions::Program;
use crate::support::*;

/// 一个扩展包的清单：程序 `program`，名字 `name`，`features` 是 `[features]` 那几段（空的就是整个包算一个功能）。
fn extension(home: &Home, id: &str, program: &Program, name: &str, features: &str) {
    home.write(
        &format!("packages/{id}/package.toml"),
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ zh = \"{name}\", en = \"{name} EN\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{}\"\nabout = {{ en = \"X\" }}\n\n[process]\n{features}",
            program.name()
        ),
    );
}

/// 装的扩展：带几个功能的自己一组（`kind: "package"`），只带一个功能的合进「扩展」（一个扩展一行、名字是包名），都排在出厂组
/// 后面、技能前面，照包编号；同一组里重名的带包名。
#[tokio::test]
async fn extensions_come_after_the_shipped_groups() {
    let home = Home::new();
    let program = Program::new();
    let twins = "\n[features.chat]\nname = { zh = \"聊天\" }\n\n[features.peek]\nname = { zh = \"聊天\" }\n\n[features.admin]\nname = { zh = \"群管理\" }\n";
    extension(&home, "zeta", &program, "接平台", twins);
    extension(&home, "alpha", &program, "单个甲", "");
    extension(
        &home,
        "beta",
        &program,
        "单个乙",
        "\n[features.lone]\nname = { zh = \"独一个\" }\n",
    );
    let mut client = connected(&home, "zh-CN").await;
    let full = get(&mut client, "full").await;
    let order: Vec<(String, String)> = heads(&full)
        .into_iter()
        .map(|(kind, id, ..)| (kind, id))
        .collect();
    let at = |id: &str| {
        order
            .iter()
            .position(|one| one.1 == id)
            .unwrap_or_else(|| panic!("有 {id}：{order:?}"))
    };
    assert_eq!(
        order[at("research") + 1],
        ("package".into(), "zeta".into()),
        "{order:?}"
    );
    assert_eq!(
        order[at("zeta") + 1],
        ("extensions".into(), "extensions".into()),
        "{order:?}"
    );
    assert_eq!(
        order[at("extensions") + 1],
        ("skills".into(), "skills".into()),
        "{order:?}"
    );
    let zeta = section(&full, "zeta");
    assert_eq!(
        (&zeta["name"], &zeta["on"], &zeta["total"]),
        (&json!("接平台"), &json!(3), &json!(3))
    );
    assert_eq!(
        rows(zeta),
        [
            (
                "chat".into(),
                "聊天 (接平台)".into(),
                true,
                "default".into()
            ),
            (
                "peek".into(),
                "聊天 (接平台)".into(),
                true,
                "default".into()
            ),
            ("admin".into(), "群管理".into(), true, "default".into()),
        ],
        "重名的带包名，不重名的不带"
    );
    let extensions = section(&full, "extensions");
    assert_eq!(extensions["name"], "扩展");
    assert_eq!(
        rows(extensions),
        [
            ("alpha".into(), "单个甲".into(), true, "default".into()),
            ("lone".into(), "单个乙".into(), true, "default".into()),
        ],
        "一个扩展一行，名字是包名，照包编号"
    );
}

/// 没有单功能扩展的，「扩展」这一组不列。
#[tokio::test]
async fn no_single_feature_extension_no_extensions_section() {
    let home = Home::new();
    let mut client = connected(&home, "zh-CN").await;
    let full = get(&mut client, "full").await;
    assert!(
        !heads(&full)
            .iter()
            .any(|head| head.0 == "extensions" || head.0 == "package"),
        "{full}"
    );
}

/// 名字照连接的语言：组名、功能名、工具名、末尾三组的名字。
#[tokio::test]
async fn names_follow_the_connection_language() {
    let home = Home::new();
    let program = Program::new();
    extension(&home, "alpha", &program, "单个甲", "");
    let mut client = connected(&home, "en-US").await;
    let full = get(&mut client, "full").await;
    let names: Vec<String> = heads(&full).into_iter().map(|head| head.2).collect();
    assert_eq!(
        names,
        [
            "Base system",
            "Tasks and schedules",
            "Injected environment",
            "Persona",
            "Research",
            "Extensions",
            "Skills",
            "Scripts",
            "MCP servers"
        ]
    );
    assert_eq!(item(&full, "write")["name"], "New file");
    assert_eq!(
        item(&full, "clicall")["name"],
        "Call tools from the command line"
    );
    assert_eq!(item(&full, "alpha")["name"], "单个甲 EN");
    assert_eq!(item(&full, "knowledge")["tools"][0]["name"], "Search");
    let mut client = connected(&home, "ja-JP").await;
    let full = get(&mut client, "full").await;
    assert_eq!(section(&full, "environment")["name"], "注入する環境情報");
    assert_eq!(section(&full, "extensions")["name"], "拡張");
    assert_eq!(item(&full, "write")["name"], "新規ファイル");
}
