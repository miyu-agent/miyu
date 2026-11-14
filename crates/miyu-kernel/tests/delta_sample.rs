//! `model.delta` 的样本（`docs/designs/samples/transient/model.delta.jsonl`，`03-事件模型.md` 第五节）：从
//! `transient_sample.rs` 拆出来的（那边到了行数上限）。
//!
//! - 样本会话里 44 号请求的回复，一段段推给头的样子：代码里造出同样的几条，写出去和样本一字不差；
//! - 这几段增量交给累积器，拼出来的就是样本里 45 号回复的内容块：推给头的和写进日志的对得上；
//! - 193 号请求里供应商自己跑了一次搜索：开始和结果推给头的两条，结果就是 194 号回复里落盘的那一份（施工 HT-1）。

use miyu_kernel::accumulate::{Accumulator, Delta, Kind, cut_outcome};
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, ModelDelta, Piece, Transient, TransientBody};
use miyu_kernel::id::{CommandId, ModelName, ProviderId, Seq, TurnId};
use miyu_kernel::origin::{By, Model};
use miyu_kernel::time::Timestamp;

use crate::transient_sample::lines;

/// 44 号请求的回复，驱动交来的增量和它们到的时刻，照先后。
fn stream() -> Vec<(&'static str, Delta)> {
    vec![
        (
            "2026-09-25T07:04:05.942Z",
            Delta::Start {
                index: 0,
                kind: Kind::Text,
            },
        ),
        (
            "2026-09-25T07:04:06.210Z",
            Delta::Text {
                index: 0,
                text: "我先".to_string(),
            },
        ),
        (
            "2026-09-25T07:04:06.480Z",
            Delta::Text {
                index: 0,
                text: "看一下目录。".to_string(),
            },
        ),
        ("2026-09-25T07:04:06.481Z", Delta::End { index: 0 }),
        (
            "2026-09-25T07:04:06.690Z",
            Delta::Start {
                index: 1,
                kind: Kind::ToolCall {
                    name: "read".to_string(),
                },
            },
        ),
        (
            "2026-09-25T07:04:07.320Z",
            Delta::Text {
                index: 1,
                text: r#"{"path":"src"}"#.to_string(),
            },
        ),
        ("2026-09-25T07:04:07.880Z", Delta::End { index: 1 }),
    ]
}

/// 一段增量推给头的样子：42 号回合，deepseek 说的，由 `cmd-7f3a` 引起。
fn pushed(at: &str, delta: &Delta) -> Transient {
    pushed_in((42, 44, "cmd-7f3a"), at, delta)
}

/// 同 [`pushed`]，第 `turn` 号回合里看到第 `seen` 条为止的那次请求，由 `cause` 引起。
fn pushed_in((turn, seen, cause): (u64, u64, &str), at: &str, delta: &Delta) -> Transient {
    let (index, piece) = match delta {
        Delta::Start { index, kind } => (*index, Piece::Start(kind.clone())),
        Delta::Text { index, text } => (*index, Piece::Text(text.clone())),
        Delta::Outcome {
            index,
            text,
            failed,
        } => (*index, Piece::Outcome(cut_outcome(text.clone(), *failed))),
        Delta::End { index } => (*index, Piece::End),
        Delta::Private { .. } | Delta::Truncated => panic!("私有数据、写到上限都不推"),
    };
    Transient {
        at: Timestamp::parse(at).expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(turn).expect("合法的序号"))),
        by: By::Model(Model {
            endpoint: ProviderId::parse("deepseek").expect("供应商的名字合写法"),
            model: ModelName::parse("deepseek-v4").expect("模型的名字合写法"),
        }),
        cause: Some(CommandId::parse(cause).expect("命令编号合写法")),
        body: TransientBody::ModelDelta(ModelDelta {
            seen: Seq::new(seen).expect("合法的序号"),
            index,
            piece,
        }),
    }
}

#[test]
fn the_samples_are_written_exactly() {
    let lines = lines("transient/model.delta.jsonl");
    let stream = stream();
    assert_eq!(
        lines.len(),
        stream.len() + hosted_stream().len(),
        "样本和增量一样多"
    );
    for (line, (at, delta)) in lines.iter().zip(&stream) {
        assert_eq!(&pushed(at, delta).to_line(), line);
    }
}

/// 194 号回复里那次供应商侧工具（施工 HT-1）推给头的两条：开始、结果。
fn hosted_stream() -> Vec<(&'static str, Delta)> {
    vec![
        (
            "2026-11-14T10:30:01.200Z",
            Delta::Start {
                index: 0,
                kind: Kind::HostedTool {
                    name: "web_search".to_string(),
                },
            },
        ),
        (
            "2026-11-14T10:30:02.800Z",
            Delta::Outcome {
                index: 0,
                text: "1. EditorConfig Specification - https://spec.editorconfig.org/".to_string(),
                failed: false,
            },
        ),
    ]
}

/// 供应商侧工具的两条（施工 HT-1）：写出去和样本最后两行一字不差；推的结果就是 194 号回复里落盘的那一份。
#[test]
fn the_hosted_tool_samples_are_written_exactly() {
    let pushed_lines = lines("transient/model.delta.jsonl");
    let hosted = hosted_stream();
    let tail = &pushed_lines[pushed_lines.len() - hosted.len()..];
    let pushed: Vec<Transient> = hosted
        .iter()
        .map(|(at, delta)| pushed_in((193, 193, "cmd-b8c4"), at, delta))
        .collect();
    for (line, transient) in tail.iter().zip(&pushed) {
        assert_eq!(&transient.to_line(), line);
    }
    let reply = lines("events/message.assistant.jsonl")
        .iter()
        .map(|line| Event::from_line(line).expect("样本读得出来"))
        .find(|event| event.seq.get() == 194)
        .expect("样本会话里有 194 号回复");
    let Body::MessageAssistant(reply) = reply.body else {
        panic!("194 号应该是回复");
    };
    let Block::HostedTool(landed) = &reply.blocks[0] else {
        panic!("194 号回复的第一块是供应商侧工具");
    };
    let TransientBody::ModelDelta(ModelDelta {
        piece: Piece::Outcome(outcome),
        ..
    }) = &pushed[1].body
    else {
        panic!("第二条是结果");
    };
    assert_eq!(landed.outcome.as_ref(), Some(outcome));
}

#[test]
fn the_pushed_pieces_add_up_to_the_reply_in_the_log() {
    let mut accumulator = Accumulator::default();
    for (_, delta) in stream() {
        accumulator.apply(delta).expect("样本的增量对得上");
    }
    let reply = lines("events/message.assistant.jsonl")
        .iter()
        .map(|line| Event::from_line(line).expect("样本读得出来"))
        .find(|event| event.seq.get() == 45)
        .expect("样本会话里有 45 号回复");
    let Body::MessageAssistant(reply) = reply.body else {
        panic!("45 号应该是回复");
    };
    let seq = Seq::new(45).expect("45 是合法的序号");
    assert_eq!(accumulator.finish(seq), reply.blocks);
}
