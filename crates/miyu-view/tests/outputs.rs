//! 工具那一步带结果原文的前面一段（施工 9-8 十四补，终端换视图流要的）：最多 100 行、8 KiB，切在字上，切了的带 `output_cut`。

use miyu_kernel::testkit::{Line, Play, Stage};

use crate::support::*;

/// 跑一条命令，结果是 `play`，交回那一步。
fn step(play: Play) -> serde_json::Value {
    let mut stage: Stage = stage();
    stage.model([
        Line::calls("", &[("shell", r#"{"command":"ls","description":"List"}"#)]),
        Line::says("ok"),
    ]);
    stage.tools([play]);
    stage.say("list");
    let entries = same(&stage);
    json(of_kind(&entries, "tool")[0])
}

#[test]
fn a_short_result_comes_whole() {
    let tool = step(Play::done("a\nb\nc\n"));
    assert_eq!(tool["output"], "a\nb\nc\n", "{tool}");
    assert!(tool.get("output_cut").is_none(), "{tool}");
}

#[test]
fn a_result_of_exactly_the_line_limit_is_not_cut() {
    let text = "x\n".repeat(100);
    let tool = step(Play::done(&text));
    assert_eq!(tool["output"], text.as_str());
    assert!(tool.get("output_cut").is_none(), "{tool}");
}

#[test]
fn a_long_result_is_cut_at_a_hundred_lines() {
    let text: String = (1..=150).map(|n| format!("line {n}\n")).collect();
    let tool = step(Play::done(&text));
    let output = tool["output"].as_str().expect("有 output");
    assert_eq!(output.lines().count(), 100, "{output}");
    assert!(output.ends_with("line 100"), "{output}");
    assert_eq!(tool["output_cut"], true);
}

#[test]
fn a_wide_result_is_cut_at_eight_kib_on_a_character() {
    // 一个字三个字节，8192 不是 3 的倍数：切点落在字中间，要退到字的边界。
    let text = "字".repeat(4000);
    let tool = step(Play::done(&text));
    let output = tool["output"].as_str().expect("有 output");
    assert_eq!(output.len(), 8190, "退到前一个字的边界");
    assert!(output.chars().all(|c| c == '字'));
    assert_eq!(tool["output_cut"], true);
}

#[test]
fn a_failed_step_carries_what_went_wrong() {
    let tool = step(Play::Fails("no such file".to_string()));
    assert_eq!(tool["state"], "error");
    assert_eq!(tool["output"], "no such file");
}

#[test]
fn a_result_without_text_has_no_output() {
    let tool = step(Play::done(""));
    assert!(tool.get("output").is_none(), "{tool}");
}
