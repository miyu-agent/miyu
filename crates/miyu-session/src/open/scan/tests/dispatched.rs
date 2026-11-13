//! 派子代理的账（施工 V-37 上）：载入时一条条记的（[`Dispatched`]），和原来收空子会话时另把父会话的整份日志读一遍攒的
//! （照抄留在这里的 [`old`]）一样：同一份日志切成几条一块喂，交回的「要不要认、记下了哪些子会话」逐个相等。日志照种子造：
//! 派子代理的调用（现在的名字、以前的 `agent`）和别的调用混着，一次回复里调零到三次；结果对得上调用的居多，也有对不上的、
//! 还没有结果的；结果里的 `job.started` 带子会话、不带子会话（后台命令）、fork 出来的，一条结果里零到两个。

use std::collections::BTreeSet;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::SessionId;
use miyu_tool::is_subagent;

use super::super::{Dispatched, Scan};

/// 原来的认法（施工 7-8 起，V-37 上以前在 `miyu-endpoint` 的 `sessions/orphans.rs`），照抄攒三样的那一段：整份日志里派子代理
/// 的调用都在带着子会话的 `job.started` 的调用里的不用认（原来到这里就交回空的），不然交回记下了的子会话（原来照它在会话表里
/// 筛）。
fn old(events: &[Event]) -> Option<BTreeSet<SessionId>> {
    let mut calls = BTreeSet::new();
    let mut started_calls = BTreeSet::new();
    let mut started = BTreeSet::new();
    for event in events {
        match &event.body {
            Body::MessageAssistant(reply) => {
                calls.extend(reply.blocks.iter().filter_map(|block| match block {
                    Block::ToolCall(call) if is_subagent(&call.name) => Some(call.call_id),
                    _ => None,
                }))
            }
            Body::ToolResult(result) => {
                for effect in &result.effects {
                    if let Effect::JobStarted(job) = effect
                        && let Some(session) = &job.session
                    {
                        started_calls.insert(result.call_id);
                        started.insert(session.clone());
                    }
                }
            }
            _ => {}
        }
    }
    if calls.is_subset(&started_calls) {
        return None;
    }
    Some(started)
}

/// 照种子走的随机数（xorshift）。
struct Dice(u64);

impl Dice {
    /// `0..n` 里的一个。
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % n as u64).expect("比 n 小")
    }
}

/// 调用的名字：只有 `subagent` 是派子代理的，别的不是（`agent` 是它改名以前的名字，不再认，施工 AUD-3）。
const NAMES: [&str; 5] = ["subagent", "agent", "shell", "read", "subagents"];

/// 第 `seq` 条，种类 `kind`、`body`。
fn line(seq: usize, kind: &str, body: &str) -> Event {
    serde_json::from_str(&format!(
        r#"{{"seq":{seq},"at":"2026-10-21T01:00:00.000Z","kind":"{kind}","by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .expect("合写法")
}

/// 第 `n` 个子会话的编号：只有六个，好让同一个子会话被记好几回。
fn child(n: usize) -> String {
    format!("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d{n:02}")
}

/// 结果里的一个 `job.started`：带子会话的、后台命令（不带）、fork 出来的。
fn job(dice: &mut Dice, n: usize) -> String {
    match dice.below(3) {
        0 => format!(
            r#"{{"kind":"job.started","job":"j{n}","what":"agent","title":"t","session":"{}"}}"#,
            child(dice.below(6))
        ),
        1 => format!(r#"{{"kind":"job.started","job":"j{n}","what":"command","title":"t"}}"#),
        _ => format!(
            r#"{{"kind":"job.started","job":"j{n}","what":"agent","title":"t","session":"{}","fork":true}}"#,
            child(dice.below(6))
        ),
    }
}

/// 一份日志，一到四十来条。
fn log(dice: &mut Dice) -> Vec<Event> {
    let mut events = vec![line(1, "message.user", r#"{"blocks":[]}"#)];
    let mut calls: Vec<String> = Vec::new();
    for _ in 0..=dice.below(40) {
        let seq = events.len() + 1;
        let event = match dice.below(4) {
            0 => line(seq, "message.user", r#"{"blocks":[]}"#),
            1 => {
                let blocks: Vec<String> = (1..=dice.below(4))
                    .map(|k| {
                        let id = format!("call_{seq}_{k}");
                        calls.push(id.clone());
                        let name = NAMES[dice.below(NAMES.len())];
                        format!(r#"{{"type":"tool_call","call_id":"{id}","name":"{name}","args":"{{}}"}}"#)
                    })
                    .collect();
                let blocks = blocks.join(",");
                line(
                    seq,
                    "message.assistant",
                    &format!(r#"{{"blocks":[{blocks}],"seen":{}}}"#, seq - 1),
                )
            }
            _ => {
                let id = match calls.is_empty() || dice.below(8) == 0 {
                    true => format!("call_{}_9", 900 + dice.below(5)),
                    false => calls[dice.below(calls.len())].clone(),
                };
                let effects: Vec<String> = (0..dice.below(3)).map(|_| job(dice, seq)).collect();
                let effects = effects.join(",");
                let body = format!(
                    r#"{{"call_id":"{id}","status":"ok","blocks":[],"effects":[{effects}]}}"#
                );
                line(seq, "tool.result", &body)
            }
        };
        events.push(event);
    }
    events
}

#[test]
fn piece_by_piece_tells_what_reading_the_whole_log_did() {
    let mut dice = Dice(0x5eed_2026_1102);
    let (mut none, mut some, mut some_empty) = (0, 0, 0);
    for _ in 0..3000 {
        let log = log(&mut dice);
        let whole = old(&log);
        for size in [1, 2, 7, log.len()] {
            let mut scan = Scan::default();
            for piece in log.chunks(size) {
                scan.note(piece);
            }
            assert_eq!(scan.dispatched.recorded(), whole, "{size} 条一块：{log:?}");
        }
        match &whole {
            None => none += 1,
            Some(started) if started.is_empty() => some_empty += 1,
            Some(_) => some += 1,
        }
    }
    // 三种都要碰到够多：不用认的、要认而一个都没记下的、要认且记下了几个的。
    assert!(
        none > 50 && some_empty > 50 && some > 50,
        "{none} / {some_empty} / {some}"
    );
}

#[test]
fn nothing_dispatched_needs_no_sweep() {
    assert_eq!(Dispatched::default().recorded(), None);
}
