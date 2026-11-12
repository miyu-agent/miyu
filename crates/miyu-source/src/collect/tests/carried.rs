//! 收录时写进索引条目的知识库、许可证（施工 PV-3）：`provides` 列出包里 `kb/` 下编号写法对的目录（`kb.toml` 可以没有，同
//! 核心认包里的库），`license` 照清单原样；没带库的没有 `knowledge`，没写许可证的这一格不出。

use super::*;
use crate::test_support::put;

/// 一个带知识库的包文件：编号 `lore`，清单 `[package]` 接上 `more`。
fn with_kb(root: &std::path::Path, more: &str) -> (String, Vec<u8>) {
    let folder = root.join("src").join("lore");
    put(
        &folder,
        "package.toml",
        format!("[package]\nversion = \"1.0\"\nname = {{ en = \"Lore\" }}\n{more}\n").as_bytes(),
    );
    put(&folder, "kb/lighthouse/kb.toml", b"name = \"Lighthouse\"\n");
    put(&folder, "kb/lighthouse/keeper.md", b"# Keeper\n");
    // 没有 `kb.toml` 的照样是一个库（`docs/blueprint/knowledge.md`：`kb.toml` 可以没有）。
    put(&folder, "kb/atlas/map.md", b"# Map\n");
    // 编号写法不对的目录、`kb/` 下的文件都不算库。
    put(&folder, "kb/Bad Name/kb.toml", b"name = \"Bad\"\n");
    put(&folder, "kb/9lives/kb.toml", b"");
    put(&folder, "kb/readme.md", b"not a library\n");
    let name = crate::archive::file_name("lore", "1.0", None);
    let out = root.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    crate::archive::pack(&folder, "lore", &out).expect("打得成");
    (name, std::fs::read(&out).expect("在"))
}

#[test]
fn an_entry_lists_the_knowledge_bases_and_the_license_of_its_package() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    net.release(
        "miyu-agent/lore",
        &[with_kb(scratch.path(), "license = \" CC-BY-SA-4.0 \"")],
    );
    net.release(
        "miyu-agent/plain",
        &[package(scratch.path(), "plain", "1.0", "", None)],
    );
    let got = collect(
        &rules(&["miyu-agent/lore", "miyu-agent/plain"]),
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-11-12T03:00:00Z",
    );
    assert_eq!(got.index.packages.len(), 2, "{:?}", got.notes);
    let entry = |id: &str| {
        got.index
            .packages
            .iter()
            .find(|one| one.id == id)
            .expect("收了")
    };
    assert_eq!(
        entry("lore").provides["knowledge"],
        ["atlas", "lighthouse"],
        "照名字排；写法不对的、文件不算"
    );
    assert_eq!(entry("lore").license.as_deref(), Some("CC-BY-SA-4.0"));
    assert!(!entry("plain").provides.contains_key("knowledge"));
    assert_eq!(entry("plain").license, None);
    let written = serde_json::to_value(&got.index).expect("写得成");
    let plain = written["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|one| one["id"] == "plain")
        .expect("在");
    assert!(plain.get("license").is_none(), "没写的不出这一格");
    let back = Index::parse(&serde_json::to_vec(&got.index).expect("写得成")).expect("读得回去");
    assert_eq!(back, got.index);
}
