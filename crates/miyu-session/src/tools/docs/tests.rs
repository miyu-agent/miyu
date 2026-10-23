//! 子目录的项目指令（施工 P-14 下）：交过的一样的不再交、变了的再交；压缩、撤销以后清掉重来；一次补交照 32 KiB 截。

use super::*;

fn texts() -> ProjectDocTexts {
    ProjectDocTexts {
        open: "<p path=\"{path}\">\n".to_string(),
        close: "</p>\n".to_string(),
        replaced: "replace {root}\n".to_string(),
        cut: "cut at {limit}\n".to_string(),
    }
}

fn below(docs: &[(&str, &str)]) -> Option<Below> {
    Some(Below {
        texts: texts(),
        docs: docs
            .iter()
            .map(|(path, text)| (PathBuf::from(path), text.to_string()))
            .collect(),
    })
}

fn told(block: Option<Block>) -> Option<String> {
    block.map(|block| match block {
        Block::Text(text) => text.text,
        other => panic!("是一段字：{other:?}"),
    })
}

#[test]
fn the_same_text_is_told_once_and_a_changed_one_again() {
    let mut docs = Docs::default();
    let first = told(docs.tell(below(&[("/r/sub/AGENTS.md", "one\n")])));
    assert_eq!(
        first.as_deref(),
        Some("<p path=\"/r/sub/AGENTS.md\">\none\n</p>\n")
    );
    assert_eq!(
        told(docs.tell(below(&[("/r/sub/AGENTS.md", "one\n")]))),
        None
    );
    let changed = told(docs.tell(below(&[("/r/sub/AGENTS.md", "two\n")]))).expect("变了再交");
    assert!(changed.contains("two"), "{changed}");
}

#[test]
fn compaction_and_undo_forget_what_was_told() {
    let mut docs = Docs::default();
    drop(docs.tell(below(&[("/r/sub/AGENTS.md", "one\n")])));
    docs.note(&[]);
    assert_eq!(
        told(docs.tell(below(&[("/r/sub/AGENTS.md", "one\n")]))),
        None,
        "别的事件不清"
    );
    let undone: Event = serde_json::from_str(
        r#"{"seq":9,"at":"2026-10-23T00:00:00.000Z","kind":"turn.reverted","by":{"kind":"person","account":"alice"},"body":{"turns":[3]}}"#,
    )
    .expect("合写法");
    docs.note(&[undone]);
    assert!(
        told(docs.tell(below(&[("/r/sub/AGENTS.md", "one\n")]))).is_some(),
        "撤销以后再交"
    );
}

#[test]
fn one_telling_stops_at_the_limit() {
    let mut docs = Docs::default();
    let big = "字".repeat(LIMIT / 3 + 10);
    let text = told(docs.tell(below(&[
        ("/r/a/AGENTS.md", &big),
        ("/r/a/b/AGENTS.md", "next\n"),
    ])))
    .expect("交了");
    assert!(text.ends_with(&format!("cut at {LIMIT}\n")), "截了说一句");
    assert!(!text.contains("next"), "超了的后面那份不交");
    assert_eq!(
        told(docs.tell(below(&[("/r/a/b/AGENTS.md", "next\n")]))),
        None,
        "截掉的也算交过：她读得到文件"
    );
}
