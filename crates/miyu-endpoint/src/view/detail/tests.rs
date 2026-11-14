//! 一个效果写成的一项：移进回收站的只写路径和 `trash`，读文件、派任务的不算。

use miyu_kernel::event::{FileRead, FileTrashed};
use miyu_kernel::id::ContentHash;

use super::*;

#[test]
fn a_trashed_file_has_no_diff_and_a_read_is_not_a_file() {
    let blobs = Blobs::new(std::env::temp_dir().join("miyu-detail-unused"));
    let trashed = Effect::FileTrashed(FileTrashed {
        path: "/home/me/a.txt".to_string(),
        trash: "/home/me/.local/share/Trash/files/a.txt".to_string(),
    });
    assert_eq!(
        file(&blobs, &trashed),
        Some(json!({"path": "/home/me/a.txt", "action": "trash"}))
    );
    let read = Effect::FileRead(FileRead {
        path: "/home/me/a.txt".to_string(),
        lines: None,
        hash: ContentHash::of(b"a\n"),
        skill: None,
        display_name: Default::default(),
    });
    assert_eq!(file(&blobs, &read), None);
}

/// 一条回复：一件供应商侧工具 `call_7_1`（结果照 `outcome`）、一个工具调用 `call_7_2`。
fn hosted_reply(outcome: &str) -> Event {
    let line = format!(
        r#"{{"seq":7,"at":"2026-11-14T10:30:04.000Z","kind":"message.assistant","turn":5,"by":{{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}},"body":{{"blocks":[{{"type":"tool_call","call_id":"call_7_1","name":"read","args":"{{}}"}},{{"type":"hosted_tool","call_id":"call_7_2","name":"web_search","args":"{{}}"{outcome}}}],"seen":6}}}}"#
    );
    Event::from_line(&line).unwrap_or_else(|e| panic!("{e}\n{line}"))
}

fn call(id: &str) -> CallId {
    CallId::parse(id).expect("调用编号")
}

/// 施工 HT-3：供应商侧工具的整份结果照回复里那一块交，截过的照实带 `cut`；没结果的、别的调用的编号找不到它。
#[test]
fn a_hosted_tool_answers_with_the_outcome_in_its_block() {
    let events = vec![hosted_reply(
        r#","outcome":{"text":"1. Spec - https://spec.example/","cut":true}"#,
    )];
    let outcome = hosted_outcome(&events, call("call_7_2")).expect("找得到");
    assert_eq!(outcome.text, "1. Spec - https://spec.example/");
    assert!(outcome.cut);
    assert_eq!(
        hosted_outcome(&events, call("call_7_1")),
        None,
        "工具调用不是它"
    );
    assert_eq!(hosted_outcome(&events, call("call_8_2")), None);
    let events = vec![hosted_reply("")];
    assert_eq!(
        hosted_outcome(&events, call("call_7_2")),
        None,
        "没结果的没有"
    );
}
