use std::path::Path;

use miyu_config::package;

use super::*;
use crate::packages::Layer;
use crate::test_support::Scratch;

/// 在 `dir` 下造一个包文件夹 `id`：清单 `manifest`，外加 `files`（相对包文件夹的路径、内容）。交回读成了的 [`Found`]。
fn package(dir: &Path, id: &str, manifest: &str, files: &[(&str, &str)]) -> Found {
    let folder = dir.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    let path = folder.join("package.toml");
    std::fs::write(&path, manifest).expect("写得进");
    for (file, text) in files {
        let file = folder.join(file);
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得了");
        std::fs::write(file, text).expect("写得进");
    }
    Found {
        id: id.to_string(),
        layer: Layer::Home,
        path,
        read: package::read(manifest).map_err(crate::packages::Issue::Wrong),
    }
}

const BUNDLE: &str = "[package]\nname = { en = \"Kit\" }\n";
const SKILL: &str = "---\nname: pdf\ndescription: Read PDFs.\n---\nbody\n";
const SCRIPT: &str = "#!/bin/sh\n# Description: Echo.\n# Parameters: {\"type\": \"object\", \"properties\": {}}\necho\n";

fn check(new: &Found, installed: &[Found]) -> Vec<(&'static str, String, String)> {
    let manifest = new.read.as_ref().expect("读成了");
    conflicts(&new.id, manifest, &new.files_dir(), installed)
        .into_iter()
        .map(|one| (one.kind.as_str(), one.name, one.package))
        .collect()
}

#[test]
fn each_shared_name_is_found_once_per_package_and_kind() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let tools = "[package]\nprotocol = [1, 1]\nname = { en = \"T\" }\n\n[builtin]\n\n[features.files]\nname = { en = \"F\" }\ntools = [\"read\"]\n";
    let service =
        "[package]\nname = { en = \"S\" }\n\n[[mcp]]\nname = \"lone\"\ncommand = [\"server\"]\n";
    let installed = vec![
        package(dir, "base", tools, &[]),
        package(dir, "svc", service, &[]),
        package(
            dir,
            "kit",
            BUNDLE,
            &[("skills/pdf/SKILL.md", SKILL), ("kb/notes/a.md", "a")],
        ),
    ];
    let new = package(
        &dir.join("new"),
        "mine",
        "[package]\nname = { en = \"M\" }\n\n[[mcp]]\nname = \"lone\"\ncommand = [\"server\"]\n",
        &[
            ("skills/pdf/SKILL.md", SKILL),
            ("scripts/read.sh", SCRIPT),
            ("kb/notes/b.md", "b"),
        ],
    );
    assert_eq!(
        check(&new, &installed),
        vec![
            ("tool", "read".to_string(), "base".to_string()),
            ("skill", "pdf".to_string(), "kit".to_string()),
            ("mcp", "lone".to_string(), "svc".to_string()),
            ("knowledge", "notes".to_string(), "kit".to_string()),
        ]
    );
}

#[test]
fn the_same_package_and_broken_ones_are_not_compared() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let old = package(dir, "kit", BUNDLE, &[("skills/pdf/SKILL.md", SKILL)]);
    let broken = package(
        dir,
        "broken",
        "[package]\n\n[ui]\n\n[process]\n",
        &[("skills/pdf/SKILL.md", SKILL)],
    );
    let upgrade = package(
        &dir.join("new"),
        "kit",
        BUNDLE,
        &[("skills/pdf/SKILL.md", SKILL)],
    );
    assert!(
        check(&upgrade, &[old, broken]).is_empty(),
        "升级不和自己比，读不成的不算"
    );
}

#[test]
fn a_command_name_and_a_feature_id_are_their_own_kinds() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let ui = "[package]\nprotocol = [1, 1]\nname = { en = \"U\" }\n\n[command]\nname = \"xcmd\"\nprogram = \"miyu-x\"\nabout = { en = \"X\" }\n\n[ui]\nopens = []\n";
    let feature = "[package]\nprotocol = [1, 1]\nname = { en = \"P\" }\n\n[builtin]\n\n[features.share]\nname = { en = \"S\" }\n";
    let installed = vec![
        package(dir, "zui", ui, &[]),
        package(dir, "zbuilt", feature, &[]),
    ];
    let new_ui = package(&dir.join("a"), "aui", ui, &[]);
    assert_eq!(
        check(&new_ui, &installed),
        vec![("command", "xcmd".to_string(), "zui".to_string())]
    );
    let new_feature = package(&dir.join("b"), "abuilt", feature, &[]);
    assert_eq!(
        check(&new_feature, &installed),
        vec![("feature", "share".to_string(), "zbuilt".to_string())]
    );
}
