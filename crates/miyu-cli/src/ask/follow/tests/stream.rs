//! 事件流（施工 AP-3，`--format jsonl`）：喂核心的推送，标准输出一行一个事件，格照名字的字母先后排；标准错误上不印旁白。

use serde_json::{Value, json};

use super::*;

/// 标准输出的几行，一字不差。
fn streamed(messages: &[Value]) -> (Step, Vec<String>, String) {
    let plan = plan(Output::Jsonl, Language::Chinese);
    let Fed { step, out, err, .. } = feed(&plan, false, messages);
    (step, out.lines().map(ToString::to_string).collect(), err)
}

#[test]
fn a_turn_streams_its_start_thinking_text_and_end() {
    let (step, lines, err) = streamed(&a_turn("completed"));
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(err, "");
    assert_eq!(
        lines,
        [
            r#"{"turn":3,"type":"turn.started"}"#,
            r#"{"text":"想一想","type":"thinking"}"#,
            r#"{"text":"你好","type":"text"}"#,
            r#"{"text":"。","type":"text"}"#,
            r#"{"reason":"completed","text":"你好。","turn":3,"type":"turn.ended","usage":{"cache_read":40,"cache_write":0,"input":100,"output":10}}"#,
        ]
    );
}

#[test]
fn each_step_starts_and_finishes() {
    let mut turn = a_turn("completed");
    let end = turn.pop().expect("最后一条是 turn.ended");
    let args = json!({"file_path": "src/lib.rs"}).to_string();
    turn.push(event(
        "message.assistant",
        3,
        "ask-4",
        json!({"seen": 6, "blocks": [
            {"type": "text", "text": "先看看。"},
            {"type": "tool_call", "call_id": "c1", "name": "read", "args": args},
            {"type": "tool_call", "call_id": "c2", "name": "mcp_lookup", "args": "{}"},
        ]}),
    ));
    turn.push(event(
        "tool.result",
        3,
        "ask-4",
        json!({"call_id": "c1", "status": "ok", "blocks": [],
            "human": {"key": "software/basesystem/read/lines", "fields": {"count": "37"}}}),
    ));
    turn.push(event(
        "tool.result",
        3,
        "ask-4",
        json!({"call_id": "c2", "status": "error", "blocks": []}),
    ));
    turn.push(end);
    let (_, lines, _) = streamed(&turn);
    let steps: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|line| line.contains(r#""type":"step"#))
        .collect();
    assert_eq!(
        steps,
        [
            r#"{"id":"c1","title":"读取 src/lib.rs","tool":"read","type":"step"}"#,
            r#"{"id":"c2","title":"mcp_lookup","tool":"mcp_lookup","type":"step"}"#,
            r#"{"id":"c1","ok":true,"summary":"37 行","type":"step.done"}"#,
            r#"{"id":"c2","ok":false,"type":"step.done"}"#,
        ]
    );
}

#[test]
fn warnings_compaction_and_errors_are_lines_too() {
    let plan = plan(Output::Jsonl, Language::Chinese);
    let mut turn = a_turn("error");
    let end = turn.pop().expect("最后一条是 turn.ended");
    turn.push(event(
        "compaction.done",
        3,
        "ask-4",
        json!({"seen": 5, "trigger": "auto", "before": 812_300, "after": 31_000}),
    ));
    turn.push(event(
        "model.called",
        3,
        "ask-4",
        json!({"seen": 6, "endpoint": "deepseek", "result": "error",
            "error": {"class": "rate_limited", "message": "slow down"}}),
    ));
    turn.push(end);
    let wide = under(&["elsewhere"]);
    let Fed { step, out, err, .. } =
        feed_after(&plan, false, Some("helper_missing"), Some(&wide), &turn);
    assert_eq!(step, Step::Done(exit::ERROR));
    let lines: Vec<Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).expect("一行 JSON"))
        .collect();
    assert_eq!(lines[0]["type"], "warning");
    assert!(
        lines[0]["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("沙盒不可用")),
        "{out}"
    );
    assert_eq!(lines[1]["type"], "warning");
    assert!(
        lines[1]["message"]
            .as_str()
            .is_some_and(|message| message.contains("范围过大")),
        "{out}"
    );
    assert!(
        lines.contains(&json!({"after": 31_000, "before": 812_300, "type": "compaction"})),
        "{out}"
    );
    let ended = lines.last().expect("有最后一行");
    assert_eq!(ended["type"], "turn.ended");
    assert_eq!(ended["reason"], "error");
    assert_eq!(
        ended["error"],
        json!({"class": "rate_limited", "message": "slow down"})
    );
    assert_eq!(
        err, "miyu: 请求受限：slow down\n",
        "标准错误上只有出错那一句"
    );
}
