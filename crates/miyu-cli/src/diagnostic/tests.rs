//! 报错、警告、提示的写法（设计 47 第三节）。

use super::*;

#[test]
fn an_error_starts_with_the_program_and_hints_follow_on_their_own_lines() {
    let problem = Problem::new("no user interface installed")
        .hint("install one with 'miyu pkg install tui'")
        .hint("or chat without one: miyu ask \"<message>\"");
    assert_eq!(
        problem.lines(Language::English, Level::Error),
        [
            "miyu: no user interface installed",
            "hint: install one with 'miyu pkg install tui'",
            "hint: or chat without one: miyu ask \"<message>\"",
        ]
    );
    assert_eq!(
        Problem::new("没有安装界面")
            .hint("用 'miyu pkg install tui' 安装")
            .lines(Language::Chinese, Level::Error),
        ["miyu: 没有安装界面", "提示：用 'miyu pkg install tui' 安装"]
    );
}

#[test]
fn a_warning_says_so_after_the_program() {
    let problem = Problem::new("working directory ~ is too broad; using ~/work");
    assert_eq!(
        problem.lines(Language::English, Level::Warning),
        ["miyu: warning: working directory ~ is too broad; using ~/work"]
    );
    assert_eq!(
        Problem::new("工作目录 ~ 范围过大，改用 ~/work").lines(Language::Chinese, Level::Warning),
        ["miyu: 警告：工作目录 ~ 范围过大，改用 ~/work"]
    );
}

#[test]
fn words_from_elsewhere_lose_the_full_stop_and_the_capital() {
    assert_eq!(Problem::new("Owner only.").what, "owner only");
    assert_eq!(Problem::new("仅主人可用。").what, "仅主人可用");
    assert_eq!(
        Problem::new("  MCP server not found. ").what,
        "MCP server not found"
    );
    assert_eq!(Problem::new("I").what, "I");
    assert_eq!(Problem::new("").what, "");
}

#[test]
fn printing_writes_every_line() {
    let mut out = Vec::new();
    error(
        &mut out,
        Language::English,
        &Problem::new("no model configured").hint("run 'miyu setup' to add one"),
    );
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "miyu: no model configured\nhint: run 'miyu setup' to add one\n"
    );
}
