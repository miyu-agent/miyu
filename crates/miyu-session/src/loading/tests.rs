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

/// 工具名那一段照样子拆（施工 X-27）：大写、带引号写的长名字也认得，各项照 `tool`、`server` 读得回来。
#[test]
fn quoted_and_long_names_are_read() {
    let long = format!("mcp__github__{}", "x".repeat(60));
    let long_key = format!("loading.tools.{long}");
    let loading = Loading::of(&values(&[
        ("loading.tools.\"my tool\"", false),
        (long_key.as_str(), true),
        ("loading.mcp.github", false),
    ]));
    assert_eq!(loading.tool("my tool"), Some(false));
    assert_eq!(loading.tool(&long), Some(true));
    assert_eq!(loading.server("github"), Some(false));
    assert_eq!(loading.tool("github"), None);
}
