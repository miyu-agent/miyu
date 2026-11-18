//! 施工 RM-1 拆掉的那个功能拆干净了（2026-11-18 项目主人定彻底移除，设计 70 已作废）：源码、资源里不许再出现它的名字。扫 `crates/`、`resources/`、
//! `xtask/` 下的文本文件，命中一处就红。文档不扫：施工记录、作废的设计稿是历史，照原样留着。
//!
//! 名字拆成两半拼起来：这个文件自己也在被扫的目录里，写成整词会扫到自己。

use std::path::{Path, PathBuf};

/// 仓库根。
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// 不许出现的词：工具、协议方法、内核事件、内置包、标签、命令、记忆里的那一类、定时表的那一种。
fn banned() -> Vec<String> {
    [
        ("end_", "life"),
        ("note_", "change"),
        ("base-", "life"),
        ("life.", "start"),
        ("life.", "kill"),
        ("life.", "ended"),
        ("life.", "list"),
        ("life-", "round"),
        ("life-", "doing"),
        ("`/li", "fe`"),
        ("/li", "fe new"),
        ("/li", "fe kill"),
        ("Loop::", "Life"),
        ("Life", "Started"),
        ("life_", "ended"),
        ("life_", "context"),
        ("推演", "人生"),
        ("人生", "会话"),
        ("人生", "循环"),
        ("推演", "方法"),
    ]
    .iter()
    .map(|(head, tail)| format!("{head}{tail}"))
    .collect()
}

/// `dir` 下的文本文件，连子目录；`target`、隐藏目录不进。
fn texts(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("读得了目录项").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if path.is_dir() {
            if name != "target" && !name.starts_with('.') {
                texts(&path, found);
            }
            continue;
        }
        let ext = path.extension().and_then(|ext| ext.to_str());
        if matches!(ext, Some("rs" | "toml" | "txt" | "json" | "md" | "jsonl")) {
            found.push(path);
        }
    }
}

#[test]
fn nothing_of_the_life_feature_is_left_in_source_or_resources() {
    let mut files = Vec::new();
    for dir in ["crates", "resources", "xtask"] {
        texts(&repo().join(dir), &mut files);
    }
    assert!(files.len() > 1000, "扫到了源码：{} 个文件", files.len());
    let banned = banned();
    let mut hits = Vec::new();
    for path in files {
        // 模型目录是别处抓来的一大份数据，里面的字不归我们管。
        if path.ends_with("resources/models/models-dev.json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (at, line) in text.lines().enumerate() {
            for word in &banned {
                if line.contains(word.as_str()) {
                    hits.push(format!("{}:{}: {word}", path.display(), at + 1));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "还有 {} 处：\n{}",
        hits.len(),
        hits.join("\n")
    );
}
