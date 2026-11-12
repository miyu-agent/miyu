//! 压缩线的百分数换得了（施工 CT-1，`snapshot/compaction.rs`）：只动那一个数，造出的策略照新的；system、工具面一字不变；没有
//! 压缩的数、没给数的照旧。

use super::*;

#[test]
fn only_the_line_percent_changes() {
    let snapshot = engineer();
    assert_eq!(snapshot.line_percent(), Some(LINE_PERCENT));
    let relined = snapshot.clone().with_line(Some(50));
    assert_eq!(relined.line_percent(), Some(50));
    assert_eq!(
        relined.policy().unwrap().compaction.unwrap().line_percent,
        50,
        "内核拿到的照新的"
    );
    let back = relined.clone().with_line(Some(LINE_PERCENT));
    assert_eq!(back, snapshot, "换回来和原来的一样");
    assert_eq!(
        (&relined.system, &relined.tools),
        (&snapshot.system, &snapshot.tools)
    );
    assert_eq!(snapshot.clone().with_line(None), snapshot, "没给数的照旧");
    let mut bare = snapshot.clone();
    bare.compaction = None;
    assert_eq!(bare.clone().with_line(Some(50)), bare, "没有压缩的数的照旧");
    assert_eq!(bare.line_percent(), None);
}
