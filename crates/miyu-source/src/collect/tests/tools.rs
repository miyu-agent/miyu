//! 收录时写进索引条目的 `tools`（施工 PV-1）：带脚本的包列出它的工具，没有工具的包这一格不出、读旧索引照旧读得进。

use super::*;
use crate::test_support::put;
use crate::tools::Tool;

/// 一个带脚本的包文件：编号 `id`，`scripts/<file>` 写 `text`。
fn with_script(root: &std::path::Path, id: &str, file: &str, text: &str) -> (String, Vec<u8>) {
    let folder = root.join("src").join(id);
    put(
        &folder,
        "package.toml",
        format!("[package]\nversion = \"1.0\"\nname = {{ en = \"{id}\" }}\n").as_bytes(),
    );
    put(&folder, &format!("scripts/{file}"), text.as_bytes());
    let name = crate::archive::file_name(id, "1.0", None);
    let out = root.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    crate::archive::pack(&folder, id, &out).expect("打得成");
    (name, std::fs::read(&out).expect("在"))
}

#[test]
fn an_entry_lists_the_tools_its_package_brings() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let script = "#!/usr/bin/env python3\n# Description: Look up a game.\n# Display name: Game crack status\n# 显示名称: 游戏破解状态查询\n";
    net.release(
        "miyu-agent/crack",
        &[with_script(
            scratch.path(),
            "crack",
            "crack_search.py",
            script,
        )],
    );
    net.release(
        "miyu-agent/plain",
        &[package(scratch.path(), "plain", "1.0", "", None)],
    );
    let got = collect(
        &rules(&["miyu-agent/crack", "miyu-agent/plain"]),
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-11-03T03:00:00Z",
    );
    let crack = got
        .index
        .packages
        .iter()
        .find(|one| one.id == "crack")
        .expect("收了");
    assert_eq!(
        crack.tools,
        [Tool {
            name: "crack_search".to_string(),
            title: [
                ("en".to_string(), "Game crack status".to_string()),
                ("zh".to_string(), "游戏破解状态查询".to_string()),
            ]
            .into(),
            from: "script".to_string(),
        }]
    );
    let json = serde_json::to_value(crack).expect("写得成");
    assert_eq!(
        json["tools"],
        serde_json::json!([{"name": "crack_search", "title": {"en": "Game crack status", "zh": "游戏破解状态查询"}, "from": "script"}])
    );
    let plain = got
        .index
        .packages
        .iter()
        .find(|one| one.id == "plain")
        .expect("收了");
    assert!(plain.tools.is_empty());
    let json = serde_json::to_value(plain).expect("写得成");
    assert!(json.get("tools").is_none(), "没有工具的不出这一格：{json}");
    // 旧索引没有这一格：照旧读得进、读成空的。
    let old: crate::index::Entry = serde_json::from_value(json).expect("读得进");
    assert!(old.tools.is_empty());
}
