//! 只解要的种类（施工 V-39 上补）：读不成的报的错和全解一字不差；不要的种类正文不合写法、仍是合法 JSON 的不报。样本里每一种
//! 照三样深浅读的在 `tests/only_samples.rs`（要读文件，`src/` 里的测试不许 I/O）。

use crate::event::Event;

/// 读不成的照全解再读一遍：报的错一字不差。
#[test]
fn a_broken_line_is_refused_word_for_word() {
    let tool = r#"{"seq":47,"at":"2026-09-25T07:04:07.905Z","kind":"tool.result","turn":42,"by":{"kind":"tool","call_id":"call_45_1"},"body":{"call_id":"call_45_1","status":"ok","blocks":[]}}"#;
    let user = r#"{"seq":41,"at":"2026-09-25T07:04:05.123Z","kind":"message.user","by":{"kind":"person"},"body":{"blocks":[{"type":"text","text":"看"}]}}"#;
    let reply = r#"{"seq":45,"at":"2026-09-25T07:04:06.000Z","kind":"message.assistant","turn":42,"by":{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"},"body":{"blocks":[],"seen":44}}"#;
    let broken = [
        // 外壳坏了。
        tool.replace(r#""seq":47"#, r#""seq":"47""#),
        tool.replace(r#""seq":47,"#, ""),
        tool.replace(r#""at":"2026-09-25T07:04:07.905Z""#, r#""at":"昨天""#),
        tool.replace(r#""turn":42"#, r#""turn":-1"#),
        tool.replace(r#""kind":"tool.result""#, r#""kind":"Tool Result""#),
        tool.replace(r#""call_id":"call_45_1"}"#, r#""call_id":7}"#),
        tool.replace(r#","body":{"#, r#","body":{},"body":{"#),
        tool.replace(
            r#","body":{"call_id":"call_45_1","status":"ok","blocks":[]}"#,
            "",
        ),
        format!("{tool}x"),
        tool[..tool.len() - 1].to_string(),
        // 不要的种类的正文不是合法 JSON。
        tool.replace(r#""blocks":[]"#, r#""blocks":[}"#),
        tool.replace(r#""status":"ok""#, r#""status":ok"#),
        // 要的种类的正文不合写法。
        user.replace(r#""blocks":["#, r#""blocks":3,"x":["#),
        user.replace(r#""type":"text""#, r#""type":5"#),
        // 只解几格的：那几格不合写法、没有。
        reply.replace(r#""seen":44"#, r#""seen":0"#),
        reply.replace(r#","seen":44"#, ""),
        reply.replace(r#""blocks":[]"#, r#""blocks":[}"#),
        "不是 JSON".to_string(),
        String::new(),
    ];
    for line in &broken {
        let full = Event::from_line(line).expect_err(line);
        let only = Event::from_line_only(line, &[]).expect_err(line);
        assert_eq!(only.to_string(), full.to_string(), "{line}");
    }
}

/// 不要的种类正文仍是合法 JSON、只是不合那一种的写法：全解报错，只解要的种类的不报（2026-11-03 项目主人拍板）。
#[test]
fn a_skipped_body_that_is_still_json_is_not_checked() {
    let tool = r#"{"seq":47,"at":"2026-09-25T07:04:07.905Z","kind":"tool.result","turn":42,"by":{"kind":"tool","call_id":"call_45_1"},"body":{"status":5}}"#;
    let reply = r#"{"seq":45,"at":"2026-09-25T07:04:06.000Z","kind":"message.assistant","turn":42,"by":{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"},"body":{"blocks":7,"seen":44}}"#;
    for line in [tool, reply] {
        assert!(Event::from_line(line).is_err(), "{line}");
        let full: serde_json::Value = serde_json::from_str(line).expect("是 JSON");
        let only = Event::from_line_only(line, &[]).expect("不解的不查");
        assert_eq!(Some(only.body.kind()), full["kind"].as_str());
    }
    // 要了的照查。
    assert!(Event::from_line_only(tool, &["tool.result"]).is_err());
    assert!(Event::from_line_only(reply, &["message.assistant"]).is_err());
}
