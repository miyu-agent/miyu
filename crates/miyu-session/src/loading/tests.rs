//! 人改的加载方式（施工 L-3）：工具自己那一项先，MCP 的再照服务那一项，都没写的照来处。

use super::*;

fn values(pairs: &[(&str, bool)]) -> Values {
    let mut values = Values::defaults(&[]);
    for (key, set) in pairs {
        values.set(key, Value::Bool(*set));
    }
    values
}

#[test]
fn a_tool_setting_beats_its_server_and_both_beat_the_source() {
    let loading = Loading::of(&values(&[
        ("loading.tools.mcp__github__create_issue", false),
        ("loading.mcp.github", true),
        ("loading.tools.web_fetch", true),
    ]));
    assert!(
        !loading.need_load("mcp__github__create_issue", true),
        "工具自己那一项先"
    );
    assert!(
        loading.need_load("mcp__github__list_issues", false),
        "再照服务"
    );
    assert!(loading.need_load("web_fetch", false), "自带的也能改");
    assert!(loading.need_load("mcp__other__x", true), "没写的照来处");
    assert!(!loading.need_load("read", false));
}

#[test]
fn only_the_two_prefixes_count() {
    let loading = Loading::of(&values(&[("compaction.prepare", true)]));
    assert_eq!(loading, Loading::default());
}
