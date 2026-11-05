//! 人用 `!` 跑的命令导出成 Markdown（施工 SH-2，`miyu_view::markdown`）：`read` 写成 `$ <命令>`、怎么结束的、用时、输出，长的
//! 折起来；还在跑的写「还在跑」；异常结束的先写原因；`agent` 一律英文、输出截头尾；`tools: line` 不写输出；普通的话照旧。

use serde_json::json;

use miyu_view::Entry;
use miyu_view::markdown::{Options, Range, Sources, Tools};

use crate::markdown::{entry, zh};
use crate::markdown_presets::rendered;

/// 一条 `!` 命令、她回一句。
fn ran(shell: serde_json::Value) -> Vec<Entry> {
    vec![
        entry(json!({"id": "m2", "kind": "user", "text": "cat nope", "turn": 3, "shell": shell})),
        entry(
            json!({"id": "b4.0", "kind": "reply", "text": "文件不在。", "persona": null, "turn": 3}),
        ),
    ]
}

fn read(entries: &[Entry]) -> String {
    zh(entries, &Sources::default(), &[]).text
}

#[test]
fn read_writes_the_command_how_it_ended_and_its_output() {
    let text = read(&ran(
        json!({"exit": 3, "text": "nope\n", "omitted": 0, "duration_ms": 1200}),
    ));
    assert!(
        text.contains(
            "## User\n\n```console\n$ cat nope\n```\n\n退出码 3 · 1s\n\n```text\nnope\n```\n\n## Assistant\n\n文件不在。"
        ),
        "{text}"
    );
}

#[test]
fn a_long_output_folds_like_a_tool_output() {
    let output: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    let text = read(&ran(
        json!({"exit": 0, "text": output, "omitted": 0, "duration_ms": 10}),
    ));
    assert!(
        text.contains("<details>\n<summary>输出（30 行）</summary>"),
        "{text}"
    );
}

#[test]
fn a_running_command_says_so_and_an_odd_ending_says_why_first() {
    let text = read(&ran(json!({"running": true})));
    assert!(
        text.contains("$ cat nope\n```\n\n还在跑\n\n## Assistant"),
        "{text}"
    );
    let text = read(&ran(
        json!({"signal": 9, "text": "half\n", "omitted": 0, "reason": "timed_out", "duration_ms": 120000}),
    ));
    assert!(text.contains("到时被停了 · 信号 9 · 2m 00s"), "{text}");
    let text = read(&ran(
        json!({"text": "", "omitted": 0, "reason": "interrupted", "duration_ms": 300}),
    ));
    assert!(
        text.contains("```\n\n被打断了 · 1s\n\n## Assistant"),
        "{text}"
    );
}

#[test]
fn agent_is_english_and_cuts_a_long_output() {
    let output = "x".repeat(10_000);
    let options = Options {
        range: Range::All,
        ..Options::AGENT
    };
    let text = rendered(
        &ran(json!({"exit": 1, "text": output, "omitted": 0, "duration_ms": 500, "reason": "aborted"})),
        &Sources::default(),
        options,
    )
    .text;
    assert!(
        text.contains("```console\n$ cat nope\n```\n\nCut off when Miyu stopped · Exit code 1 · 1s\n\n```text\n"),
        "{text}"
    );
    assert!(!text.contains(&"x".repeat(5000)), "没截：{}", text.len());
}

#[test]
fn a_line_per_tool_leaves_the_output_out() {
    let options = Options {
        tools: Tools::Line,
        range: Range::All,
        ..Options::READ
    };
    let text = rendered(
        &ran(json!({"exit": 0, "text": "secret-ish output\n", "omitted": 0, "duration_ms": 10})),
        &Sources::default(),
        options,
    )
    .text;
    assert!(
        text.contains("$ cat nope\n```\n\n退出码 0 · 1s\n\n## Assistant"),
        "{text}"
    );
    assert!(!text.contains("secret-ish output"), "{text}");
}

#[test]
fn a_plain_message_is_still_prose() {
    let entries = vec![entry(
        json!({"id": "m2", "kind": "user", "text": "cat nope", "turn": 3}),
    )];
    let text = read(&entries);
    assert!(text.contains("## User\n\ncat nope"), "{text}");
    assert!(!text.contains("```console"), "{text}");
}
