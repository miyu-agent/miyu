//! 循环给模型看的字不说醒、睡（施工 WK-2，2026-11-17 项目主人定）：她不是每一轮之间睡下去了，一轮接一轮说「继续」
//! 「下一轮」。查 `resources/software/loop` 下所有的 `.txt` 和 `tools/*.json`，给人看的 `human/` 不在里头。

use std::path::{Path, PathBuf};

/// 不许出现的词，不分大小写。
const BANNED: [&str; 5] = ["wake", "woke", "awake", "sleep", "asleep"];

/// `dir` 下（递归）给模型看的字：`.txt`，和 `tools/` 里的 `.json`；跳过 `human/`。
fn model_facing(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("读得了目录") {
        let path = entry.expect("读得了目录项").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "human") {
                model_facing(&path, found);
            }
            continue;
        }
        let ext = path.extension().and_then(|ext| ext.to_str());
        let in_tools = path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "tools");
        if ext == Some("txt") || (ext == Some("json") && in_tools) {
            found.push(path);
        }
    }
}

#[test]
fn loop_texts_never_say_wake_or_sleep() {
    let software = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software");
    let mut files = Vec::new();
    model_facing(&software.join("loop"), &mut files);
    assert!(files.len() > 5, "找到了出厂的字：{files:?}");
    for path in files {
        let text = std::fs::read_to_string(&path)
            .expect("读得了")
            .to_lowercase();
        for word in BANNED {
            assert!(!text.contains(word), "{} 里有「{word}」", path.display());
        }
    }
}
