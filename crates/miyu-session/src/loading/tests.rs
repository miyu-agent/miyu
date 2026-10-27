//! 脚本里能不能调（施工 M-2）：工具自己那一项先，MCP 的再照服务那一项，都没写的照来处；退役的 `loading.*` 不算（施工 FL-1）。

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
    let set = values(&[
        ("clicall.tools.mcp__github__create_issue", false),
        ("clicall.mcp.github", true),
        ("clicall.tools.read", true),
        ("loading.tools.web_fetch", true),
    ]);
    let clicall = Loading::clicall_of(&set);
    assert!(
        !clicall.clicall("mcp__github__create_issue", true),
        "工具自己那一项先"
    );
    assert!(
        clicall.clicall("mcp__github__list_issues", false),
        "再照服务"
    );
    assert!(clicall.clicall("read", false), "自带的也能开");
    assert!(
        !clicall.clicall("web_fetch", false),
        "退役的加载方式那一项不算"
    );
    assert!(clicall.clicall("mcp__other__x", true), "没写的照来处");
}

#[test]
fn only_the_two_prefixes_count() {
    let clicall = Loading::clicall_of(&values(&[("compaction.prepare", true)]));
    assert_eq!(clicall, Loading::default());
}

/// 工具名那一段照样子拆（施工 X-27）：大写、带引号写的长名字也认得。
#[test]
fn quoted_and_long_names_are_read() {
    let long = format!("mcp__github__{}", "x".repeat(60));
    let long_key = format!("clicall.tools.{long}");
    let clicall = Loading::clicall_of(&values(&[
        ("clicall.tools.\"my tool\"", true),
        (long_key.as_str(), true),
        ("clicall.mcp.github", false),
    ]));
    assert!(clicall.clicall("my tool", false));
    assert!(clicall.clicall(&long, false), "长名字自己那一项先于服务");
    assert!(!clicall.clicall("mcp__github__other", true), "服务那一项");
}
