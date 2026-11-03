//! 只解要的种类（施工 V-39 上补，`Event::from_line_only`）：样本里每一种照三样深浅读，外壳和全解的一样；有效历史认的、要的种类
//! 全解的一样，回复、请求只解的那几格一样，别的读成 `null`；种类写在 `body` 后面的照样分。样本住在设计文档旁边，所以放在集成
//! 测试里读文件。
//!
//! 有效历史照种类认的那几种、只解几格的两种照内核的规矩另抄一份在这里（`HISTORY`、`SEEN`），不引 `event/only.rs` 的那两张表：
//! 那边少写了、多写了，这里对不上。

use std::path::PathBuf;

use miyu_kernel::event::{Body, Event};

/// 有效历史照种类认的（`history.rs` 的 `append`，`history/undo.rs`）：全解。
const HISTORY: &[&str] = &[
    "turn.started",
    "turn.ended",
    "turn.reverted",
    "turn.unreverted",
    "message.user",
    "message.withdrawn",
    "context.compacted",
    "job.reported",
    "child.reported",
    "peer.idle",
];

/// 有效历史撤销时只看几格的。
const SEEN: &[&str] = &["message.assistant", "model.called"];

/// 样本里的每一行（`docs/designs/samples/events`，每一种都有，`tests/samples.rs` 查着）。
fn samples() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/events");
    let mut lines = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("有样本")
        .map(|entry| entry.expect("读得了").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("读得了");
        lines.extend(
            text.lines()
                .filter(|line| !line.is_empty())
                .map(String::from),
        );
    }
    lines
}

/// 外壳一样（序号、时刻、种类、回合、谁、命令编号）。
fn same_shell(a: &Event, b: &Event) {
    assert_eq!(
        (a.seq, a.at, a.turn, &a.by, &a.cause, a.body.kind()),
        (b.seq, b.at, b.turn, &b.by, &b.cause, b.body.kind())
    );
}

#[test]
fn each_kind_reads_as_deep_as_it_should() {
    let lines = samples();
    let mut seen = [0; 3];
    for line in &lines {
        let full = Event::from_line(line).expect("样本合写法");
        let only = Event::from_line_only(line, &["venue.delivered"]).expect("读得成");
        same_shell(&full, &only);
        let kind = full.body.kind();
        if HISTORY.contains(&kind) || kind == "venue.delivered" {
            assert_eq!(only, full, "{kind} 全解");
            seen[0] += 1;
        } else if SEEN.contains(&kind) {
            match (&full.body, &only.body) {
                (Body::MessageAssistant(full), Body::MessageAssistant(only)) => {
                    assert_eq!((full.seen, full.interrupted), (only.seen, only.interrupted));
                    assert!(only.blocks.is_empty());
                }
                (Body::ModelCalled(full), Body::ModelCalled(only)) => {
                    assert_eq!(
                        (full.seen, full.messages, &full.result, &full.error),
                        (only.seen, only.messages, &only.result, &only.error)
                    );
                    assert_eq!(only.usage, None);
                }
                other => panic!("{kind} 只解几格：{other:?}"),
            }
            seen[1] += 1;
        } else {
            let Body::Unknown { kind: named, body } = &only.body else {
                panic!("{kind} 不解：{:?}", only.body);
            };
            assert_eq!((named.as_str(), body.get()), (kind, "null"));
            seen[2] += 1;
        }
        // 要的写全了的，和全解一样。
        assert_eq!(
            Event::from_line_only(line, Body::KINDS).expect("读得成"),
            full
        );
    }
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
}

/// 种类写在 `body` 后面的（不是核心写的）：照样分三样。
#[test]
fn a_kind_after_its_body_reads_the_same() {
    for line in samples() {
        let full = Event::from_line(&line).expect("样本合写法");
        let Some(at) = line.find(r#","body":"#) else {
            continue;
        };
        let kind = format!(r#","kind":"{}""#, full.body.kind());
        let Some(moved) = line
            .find(&kind)
            .filter(|&k| k < at)
            .map(|k| line[..k].to_string() + &line[k + kind.len()..line.len() - 1] + &kind + "}")
        else {
            continue;
        };
        assert_eq!(Event::from_line(&moved).expect("合写法"), full, "{moved}");
        let a = Event::from_line_only(&line, &[]).expect("读得成");
        let b = Event::from_line_only(&moved, &[]).expect("读得成");
        assert_eq!(a, b, "{moved}");
    }
}
