//! 记忆日志的一行认不认：只认种类那一栏，给了会话的还要提到它。

use super::*;

const LINE: &str = r#"{"seq":3,"at":"2026-11-22T06:00:00Z","by":{"kind":"module","id":"memory"},"kind":"ext.memory.extracted","body":{"session":"01a2","upto":9,"count":1}}"#;

#[test]
fn the_kind_column_is_what_counts() {
    assert!(recorded(LINE, EXTRACTED, Some("01a2")));
    assert!(recorded(LINE, EXTRACTED, None));
    assert!(!recorded(LINE, EXTRACTED, Some("01b3")));
    assert!(!recorded(LINE, MERGED, None));
    // 正文里提到这个词的不算。
    let said = r#"{"kind":"ext.memory.saved","body":{"text":"ext.memory.merged"}}"#;
    assert!(!recorded(said, MERGED, None));
}

#[test]
fn a_missing_directory_is_empty() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-memory-dir");
    assert_eq!(read_all(&dir).unwrap(), "");
}

#[test]
fn the_sandbox_setting_and_the_script_agree_on_how_many_turns_make_one_extraction() {
    assert!(SETTINGS.contains(&format!("extract_max_turns = {TURNS}\n")));
}

/// 一轮两句（人说的、模型答的）；标准库的排序到 20 个为止用插入排序，超过了才走另一条路：一次抽取要超过 20 句，这条路
/// 才录得到。第二次抽取才读已有的摘要、和已有的记忆对。两样写成编译时的检查：改小了编不过。
const _: () = {
    assert!(TURNS * 2 > 20);
    assert!(ROUNDS >= 2);
};
