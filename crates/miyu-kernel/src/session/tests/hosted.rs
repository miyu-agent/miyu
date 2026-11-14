//! 供应商侧工具（施工 HT-1，`kernel/session.md`「收回复」）：不派、不过权限、不算「调了工具」；出错、打断、写到上限时丢
//! 工具调用不丢它，编号照留下的工具调用重排；中途订阅补推带结果。

use super::executor::*;
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::block::{HostedTool, Outcome};
use crate::event::{EndReason, ErrorClass, MessageAssistant, Piece, TransientBody};

/// 一件供应商侧工具：开始、参数、结果（有的话）、收全（`ends` 的话）。
fn hosted(index: usize, args: &str, outcome: Option<&str>, ends: bool) -> Vec<Delta> {
    let mut deltas = vec![
        Delta::Start {
            index,
            kind: Kind::HostedTool {
                name: "web_search".to_string(),
            },
        },
        Delta::Text {
            index,
            text: args.to_string(),
        },
    ];
    if let Some(text) = outcome {
        deltas.push(Delta::Outcome {
            index,
            text: text.to_string(),
            failed: false,
        });
    }
    if ends {
        deltas.push(Delta::End { index });
    }
    deltas
}

/// 一个工具调用：开始、参数、收全（`ends` 的话）。
fn tool_call(index: usize, ends: bool) -> Vec<Delta> {
    let mut deltas = vec![
        Delta::Start {
            index,
            kind: Kind::ToolCall {
                name: "read".to_string(),
            },
        },
        Delta::Text {
            index,
            text: "{}".to_string(),
        },
    ];
    if ends {
        deltas.push(Delta::End { index });
    }
    deltas
}

/// 请求 5 发出去，这些增量照先后送进去。
fn streamed(session: &mut Session, deltas: Vec<Vec<Delta>>) {
    allowing(session, sent(5));
    for delta_ in deltas.into_iter().flatten() {
        allowing(session, delta(5, 41, delta_));
    }
}

fn reply_of(events: &[Event]) -> &MessageAssistant {
    let Body::MessageAssistant(reply) = &events[0].body else {
        panic!("先写回复：{events:?}");
    };
    reply
}

fn searched(k: u32, outcome: Option<&str>) -> Block {
    Block::HostedTool(HostedTool {
        call_id: call(6, k),
        name: "web_search".to_string(),
        args: "{\"q\":1}".to_string(),
        outcome: outcome.map(|text| Outcome {
            text: text.to_string(),
            failed: false,
            cut: false,
        }),
        private: None,
    })
}

fn no_tool_ran(actions: &[Action]) {
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::GuardTool { .. } | Action::RunTool { .. })),
        "{actions:?}"
    );
}

/// 只有它和正文的回复：照没有工具调用收尾，回合照常结束，执行器一次都没被叫去跑工具。
#[test]
fn a_reply_with_only_hosted_tools_ends_the_turn() {
    let mut session = asking();
    streamed(
        &mut session,
        vec![
            hosted(0, "{\"q\":1}", Some("1. Rust"), true),
            words(1, "找到了"),
        ],
    );
    let actions = allowing(&mut session, ended(5));
    no_tool_ran(&actions);
    let events = appended_events(&actions);
    assert_eq!(
        appended(&actions),
        seqs(&[6, 7, 8]),
        "回复、model.called、turn.ended"
    );
    let reply = reply_of(&events);
    assert!(!reply.interrupted);
    assert_eq!(reply.blocks[0], searched(1, Some("1. Rust")));
    let actions = allowing(&mut session, stored(8));
    no_tool_ran(&actions);
    assert_eq!(reason_of(&events[2]), &EndReason::Completed);
}

/// 和工具调用混在一条回复里：只派工具调用，它接着工具调用编号。
#[test]
fn only_the_tool_calls_beside_it_are_run() {
    let mut session = asking();
    streamed(
        &mut session,
        vec![hosted(0, "{\"q\":1}", None, true), tool_call(1, true)],
    );
    let actions = allowing(&mut session, ended(5));
    let events = appended_events(&actions);
    assert_eq!(reply_of(&events).blocks[0], searched(2, None));
    let actions = allowing(&mut session, stored(7));
    assert_eq!(guards(&actions), [call(6, 1)]);
    assert_eq!(ran(&actions), [call(6, 1)]);
}

/// 出错断了的回复：工具调用一个不留，它还在，编号照留下的工具调用（一个没有）从 `_1` 排，账本收得下。
#[test]
fn a_failed_reply_keeps_it_and_drops_the_tool_calls() {
    let mut session = asking();
    streamed(
        &mut session,
        vec![
            tool_call(0, true),
            hosted(1, "{\"q\":1}", Some("1. Rust"), true),
        ],
    );
    let actions = allowing(&mut session, failed(5, ErrorClass::Retryable, "reset"));
    no_tool_ran(&actions);
    let reply = reply_of(&appended_events(&actions)).clone();
    assert!(reply.interrupted);
    assert_eq!(reply.blocks, [searched(1, Some("1. Rust"))]);
}

/// 被打断：没收全的它也留下、没有结果；没收全的工具调用丢掉，不占号。
#[test]
fn an_interrupted_reply_keeps_it_even_unfinished() {
    let mut session = asking();
    streamed(
        &mut session,
        vec![
            tool_call(0, true),
            hosted(1, "{\"q\":1}", None, false),
            tool_call(2, false),
        ],
    );
    let actions = allowing(&mut session, stop_with(2, at(47), Queued::Send));
    let events = appended_events(&actions);
    let reply = reply_of(&events);
    assert!(reply.interrupted);
    assert_eq!(reply.blocks[1], searched(2, None));
    assert_eq!(reply.blocks.len(), 2);
    assert!(matches!(&reply.blocks[0], Block::ToolCall(tool) if tool.call_id == call(6, 1)));
}

/// 写到输出上限：工具调用不派、不留，它留着，编号从 `_1` 排；只有它和正文的照正文截了收（不算截掉了工作）。
#[test]
fn a_truncated_reply_keeps_it_and_drops_the_tool_calls() {
    let mut session = asking();
    streamed(
        &mut session,
        vec![
            tool_call(0, true),
            hosted(1, "{\"q\":1}", Some("1. Rust"), true),
            vec![Delta::Truncated],
        ],
    );
    let actions = allowing(&mut session, ended(5));
    no_tool_ran(&actions);
    let reply = reply_of(&appended_events(&actions)).clone();
    assert!(reply.interrupted);
    assert_eq!(reply.blocks, [searched(1, Some("1. Rust"))]);
}

/// 中途订阅的补推：这一块的字后面、`end` 前面带一条结果。
#[test]
fn a_late_subscriber_gets_the_outcome() {
    let mut session = asking();
    streamed(
        &mut session,
        vec![hosted(0, "{\"q\":1}", Some("1. Rust"), true)],
    );
    let pieces: Vec<(usize, Piece)> = session
        .streaming(at(50))
        .iter()
        .map(|transient| {
            let TransientBody::ModelDelta(body) = &transient.body else {
                panic!("应该是 model.delta：{transient:?}");
            };
            (body.index, body.piece.clone())
        })
        .collect();
    assert_eq!(
        pieces,
        [
            (
                0,
                Piece::Start(Kind::HostedTool {
                    name: "web_search".to_string()
                })
            ),
            (0, Piece::Text("{\"q\":1}".to_string())),
            (
                0,
                Piece::Outcome(Outcome {
                    text: "1. Rust".to_string(),
                    failed: false,
                    cut: false
                })
            ),
            (0, Piece::End),
        ]
    );
}

/// 写到输出上限、只有正文和它的回复：算不算截掉了工作只看工具调用，照正文截了收（`reply_cut`），不算截掉了工作。
#[test]
fn a_truncated_reply_with_only_text_and_hosted_tools_is_a_cut_text() {
    let mut session = asking();
    streamed(
        &mut session,
        vec![
            words(0, "我搜了一下"),
            hosted(1, "{\"q\":1}", Some("1. Rust"), true),
            vec![Delta::Truncated],
        ],
    );
    let actions = allowing(&mut session, ended(5));
    no_tool_ran(&actions);
    let events = appended_events(&actions);
    let facts: Vec<&str> = events
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) => Some(fact.kind.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(facts, ["reply_cut"], "{events:?}");
}

/// 推给头的结果是截过的那一份，和落进块里的一样。
#[test]
fn the_pushed_outcome_is_the_cut_one() {
    use crate::accumulate::OUTCOME_BYTES;
    let mut session = asking();
    streamed(&mut session, vec![hosted(0, "{\"q\":1}", None, false)]);
    let long = format!("{}好好", "a".repeat(OUTCOME_BYTES - 1));
    let actions = session.handle(delta(
        5,
        42,
        Delta::Outcome {
            index: 0,
            text: long,
            failed: false,
        },
    ));
    let [Action::PushTransient(transient)] = actions.as_slice() else {
        panic!("推一条瞬时事件：{actions:?}");
    };
    let TransientBody::ModelDelta(body) = &transient.body else {
        panic!("应该是 model.delta：{transient:?}");
    };
    let Piece::Outcome(pushed) = &body.piece else {
        panic!("应该是结果：{body:?}");
    };
    assert!(pushed.cut);
    assert_eq!(pushed.text, "a".repeat(OUTCOME_BYTES - 1));
    session.handle(delta(5, 43, Delta::End { index: 0 }));
    let reply = reply_of(&appended_events(&allowing(&mut session, ended(5)))).clone();
    let [Block::HostedTool(landed)] = reply.blocks.as_slice() else {
        panic!("{reply:?}");
    };
    assert_eq!(landed.outcome.as_ref(), Some(pushed));
}
