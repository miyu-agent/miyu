//! 拆段：运行日志一行的读法、时区、ISO 时刻；照偏移平均、排先后。

use std::time::{Duration, UNIX_EPOCH};

use serde_json::json;

use super::*;
use crate::cpu::Cpu;

const SESSION: &str = "019a3b2c-1d2e-7f30-8a4b-5c6d7e8f9a0b";

#[test]
fn a_log_line_is_read_with_its_session_and_kind() {
    let line = format!("2026-10-31 09:00:00.250 DEBUG session  {SESSION} action kind=guard_tool");
    let parsed = Line::parse(&line, 480.0).unwrap();
    assert_eq!(parsed.session, SESSION);
    assert_eq!(parsed.label, "action guard_tool");
    assert_eq!(parsed.at, iso_ms("2026-10-31T01:00:00.250Z").unwrap());
    let ran =
        format!("2026-10-31 09:00:00.250 INFO  session  {SESSION} running call=c1 tool=shell");
    assert_eq!(Line::parse(&ran, 0.0).unwrap().label, "running shell");
    let plain = "2026-10-31 09:00:00.250 INFO  core     starting version=1 pid=3 tz=+08:00";
    let parsed = Line::parse(plain, 0.0).unwrap();
    assert_eq!(parsed.session, "");
    assert_eq!(parsed.label, "starting");
    assert_eq!(offset(plain), Some(480.0));
    assert_eq!(offset("x tz=-05:30"), Some(-330.0));
    assert_eq!(offset("x tz=0800"), None);
    assert_eq!(Line::parse("garbage", 0.0), None);
}

#[test]
fn iso_times_count_from_1970() {
    assert_eq!(iso_ms("1970-01-01T00:00:01.500Z"), Some(1500.0));
    assert_eq!(iso_ms("2000-03-01T00:00:00.000Z"), Some(951_868_800_000.0));
    assert_eq!(iso_ms("2026-10-31 01:00:00"), None);
}

fn step(call: &str, done_ms: u64, took: f64) -> Step {
    let done = UNIX_EPOCH + Duration::from_millis(done_ms);
    Step {
        call: call.to_string(),
        ms: took,
        cpu: Cpu::default(),
        done,
        arrived: done + Duration::from_secs_f64(took / 1000.0),
    }
}

fn log(at_ms: u64, what: &str) -> Line {
    Line {
        at: at_ms as f64,
        session: SESSION.to_string(),
        label: what.to_string(),
    }
}

/// 一条带一次调用的回复：内核的编号 `internal`，供应商的编号 `call` 收在 `private` 里。
fn assistant(at: &str, internal: &str, call: &str) -> serde_json::Value {
    json!({"kind": "message.assistant", "at": at, "body": {"blocks": [{"type": "text", "text": "Checking."},
        {"type": "tool_call", "call_id": internal, "name": "read", "private": {"id": call}}]}})
}

#[test]
fn marks_are_averaged_and_ordered() {
    let steps = [step("c1", 10_000, 8.0), step("c2", 20_000, 12.0)];
    let events = vec![
        assistant("1970-01-01T00:00:10.001Z", "call_1_1", "c1"),
        json!({"kind": "tool.result", "at": "1970-01-01T00:00:10.006Z", "body": {"call_id": "call_1_1", "status": "ok"}}),
        assistant("1970-01-01T00:00:20.003Z", "call_3_1", "c2"),
        json!({"kind": "tool.result", "at": "1970-01-01T00:00:20.010Z", "body": {"call_id": "call_3_1", "status": "ok"}}),
    ];
    let lines = vec![
        // 上一步留在同一毫秒里的那一行不算：从这一步的 `ended` 起。
        log(10_000, "request"),
        log(10_000, "ended"),
        log(10_002, "running"),
        log(10_002, "running"),
        log(20_000, "ended"),
        log(20_004, "running"),
        log(25_000, "running"),
        Line {
            session: "other".to_string(),
            ..log(10_003, "request")
        },
    ];
    let split = one("x", SESSION, &steps, &events, &lines);
    assert_eq!(split.steps, 2);
    let labels: Vec<(&str, usize, f64)> = split
        .marks
        .iter()
        .map(|mark| (mark.label.as_str(), mark.seen, mark.mean))
        .collect();
    assert_eq!(
        labels,
        [
            ("运行日志 ended", 2, 0.5),
            ("会话日志 message.assistant", 2, 2.5),
            ("运行日志 running", 2, 3.5),
            ("会话日志 tool.result", 2, 8.5),
            ("假模型读全下一个请求头", 2, 10.0),
        ]
    );
}

#[test]
fn a_failed_call_is_reported() {
    let events = vec![
        assistant("1970-01-01T00:00:10.001Z", "call_1_1", "c1"),
        json!({"kind": "tool.result", "body": {"call_id": "call_1_1", "status": "ok"}}),
        assistant("1970-01-01T00:00:10.003Z", "call_3_1", "c2"),
        json!({"kind": "tool.result", "body": {"call_id": "call_3_1", "status": "error", "blocks": []}}),
        assistant("1970-01-01T00:00:10.005Z", "call_5_1", "c4"),
    ];
    assert!(check(&events, "x", "c1").is_ok());
    assert!(check(&events, "x", "c2").unwrap_err().contains("error"));
    assert!(check(&events, "x", "c3").unwrap_err().contains("这次调用"));
    assert!(check(&events, "x", "c4").unwrap_err().contains("的结果"));
}
