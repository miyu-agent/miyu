//! 载入时边读边记的几样（施工 V-15）：一块块记的和整份找的一样；她看过的文件照底子、自己的先后算；换人格的几条都留下；读完
//! 照最近一次换的快照，和照着取了策略的那一份一样的不换。

use std::path::Path;

use miyu_kernel::event::{Body, Effect, Event, FileRead, ToolResult};
use miyu_kernel::id::{CallId, ContentHash, Seq, TurnId};
use miyu_kernel::origin::{By, Tool};
use miyu_kernel::time::Timestamp;

use super::*;

/// 一条事件：序号、种类、`body`。
fn event(seq: u64, kind: &str, body: &str) -> Event {
    serde_json::from_str(&format!(
        r#"{{"seq":{seq},"at":"2026-10-21T01:00:{seq:02}.000Z","kind":"{kind}","by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .expect("合写法")
}

/// 第 `seq` 条：读了 `path`，内容是 `content`。
fn read(seq: u64, path: &str, content: &[u8]) -> Event {
    let call_id = CallId::new(Seq::new(1).unwrap(), 1).unwrap();
    Event {
        seq: Seq::new(seq).unwrap(),
        at: Timestamp::from_unix_millis(0).unwrap(),
        turn: Some(TurnId::new(Seq::new(1).unwrap())),
        by: By::Tool(Tool { call_id }),
        cause: None,
        body: Body::ToolResult(ToolResult {
            call_id,
            status: miyu_kernel::event::ToolStatus::Ok,
            blocks: Vec::new(),
            duration_ms: None,
            human: None,
            effects: vec![Effect::FileRead(FileRead {
                path: path.to_string(),
                lines: None,
                hash: ContentHash::of(content),
                skill: None,
                display_name: Default::default(),
            })],
            end_turn: false,
            sent: Vec::new(),
            display_name: Default::default(),
            detail: None,
        }),
    }
}

/// 写成 JSON 的哈希。
fn json(hash: &ContentHash) -> String {
    serde_json::to_string(hash).expect("写得成")
}

/// 一份日志：换了两次快照、两次人格（一次连着快照），只换模型的一次。
fn log() -> Vec<Event> {
    let a = ContentHash::of(b"a");
    let b = ContentHash::of(b"b");
    vec![
        event(1, "message.user", r#"{"blocks":[]}"#),
        event(
            2,
            "session.policy_changed",
            &format!(r#"{{"policy":{}}}"#, json(&a)),
        ),
        event(3, "session.policy_changed", r#"{"persona":"b"}"#),
        event(4, "session.policy_changed", r#"{"model":"p/m"}"#),
        event(
            5,
            "session.policy_changed",
            &format!(r#"{{"policy":{},"persona":"c"}}"#, json(&b)),
        ),
        event(6, "message.user", r#"{"blocks":[]}"#),
    ]
}

#[test]
fn piece_by_piece_finds_what_the_whole_log_does() {
    let log = log();
    for size in [1, 2, 6] {
        let mut scan = Scan::default();
        for piece in log.chunks(size) {
            scan.note(piece);
        }
        assert_eq!(scan.policy.as_ref(), current_policy(&log), "{size} 条一块");
        assert_eq!(scan.remembered(), remembered(&log));
        assert_eq!(scan.count, log.len());
        let recasts: Vec<u64> = scan.recasts.iter().map(|event| event.seq.get()).collect();
        assert_eq!(recasts, [3, 5], "换了人格的都留下，只换模型、快照的不留");
    }
}

#[test]
fn what_she_saw_counts_the_base_first() {
    let mut scan = Scan::default();
    scan.base(&[read(1, "/w/base", b"x")]);
    scan.note(&[read(2, "/w/own", b"y")]);
    let seen = scan.seen.seen();
    assert_eq!(seen.get(Path::new("/w/base")), Some(&ContentHash::of(b"x")));
    assert_eq!(seen.get(Path::new("/w/own")), Some(&ContentHash::of(b"y")));
}

#[test]
fn the_last_switched_snapshot_is_taken_unless_it_is_the_one_in_hand() {
    let (a, b, created) = (
        ContentHash::of(b"a"),
        ContentHash::of(b"b"),
        ContentHash::of(b"created"),
    );
    let mut scan = Scan::default();
    assert_eq!(
        scan.switched(&created, &created),
        None,
        "没换过的照造会话的"
    );
    assert_eq!(scan.switched(&a, &created), Some(created.clone()));
    scan.note(&log());
    assert_eq!(scan.switched(&b, &created), None, "照着取的就是最后那一份");
    assert_eq!(scan.switched(&a, &created), Some(b));
}
