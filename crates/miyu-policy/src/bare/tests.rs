//! 裸的快照（施工 D-1 上）：去掉 system、工具和几样开关，字节里记 `bare`；造出的策略不压缩、不起标题。不裸的字节不变。

use crate::test_support::engineer;

#[test]
fn a_bare_snapshot_keeps_nothing_but_the_core_texts() {
    let snapshot = engineer();
    assert!(
        !String::from_utf8(snapshot.to_bytes())
            .expect("是字")
            .contains("\"bare\""),
        "不裸的不写这一格，字节和以前一样"
    );
    let bare = snapshot.clone().bare();
    assert!(bare.system.is_empty() && bare.tools.is_empty() && bare.demos.is_empty());
    assert_eq!(bare.persona, None);
    assert_eq!(bare.core, snapshot.core, "随核心附带的字照留");
    let text = String::from_utf8(bare.to_bytes()).expect("是字");
    assert!(text.contains(r#""bare":true"#), "{text}");
    let policy = bare.policy().expect("造得出策略");
    assert!(policy.compaction.is_none(), "不压缩");
    assert!(policy.titles.is_none(), "不起标题");
    assert!(policy.tools.is_empty(), "没有工具");
}
