//! 分叉边读边算（施工 V-39 中，`docs/construction/V-39中-支线底子边读边喂.md`）：父会话一块块读、交给内核一块块算，和以前
//! 整份读进来再算的（照抄在 `fork_streamed/old.rs`）写下的第一批逐字节一样，交回的分叉点、报的错和先后一样。
//!
//! 父会话照种子造（`fork_streamed/make.rs`，每一条都过得了账本），一千份：分几段写；有的停在一轮当中、一步刚派出调用上；
//! 有的坏了一行（分叉点以前、以后、最后那一轮里）、末尾半行、一条都没有、没有这个会话、第一条不是造会话、是子代理或分叉出来
//! 的、换的快照取不出；普通分叉、fork 子代理（派的调用在等的、不在等的）都有。

#[path = "fork_streamed/make.rs"]
mod make;
#[path = "fork_streamed/old.rs"]
mod old;

use std::fs;
use std::path::Path;

use miyu_kernel::event::{Body, Event, Forked};
use miyu_kernel::id::{CallId, CommandId, Seq, SessionId};
use miyu_session::testkit::Script;
use miyu_session::{Fork, ForkError, Within, new_id};

use crate::support::*;
use make::{Dice, Make};

/// 造多少份。
const CASES: u64 = 1000;

/// 一份怎么开头。
#[derive(Clone, Copy)]
enum Start {
    /// 主会话。
    Main,
    /// 子代理：普通分叉拒，fork 子代理能分。
    Child,
    /// 分叉出来的：都拒。
    Forked,
    /// 第一条不是造会话。
    NotCreated,
}

#[tokio::test]
async fn forking_reads_the_parent_in_blocks_and_writes_what_the_whole_read_wrote() {
    let home = Home::new();
    let made = home.create(&Script::new([])).await;
    let created = home.log(made.id())[0].clone();
    stop(&made).await;
    let policy = match &created.body {
        Body::SessionCreated(body) => body.policy.to_string(),
        _ => panic!("第一条是造会话"),
    };
    let mut outcomes = std::collections::BTreeMap::<String, usize>::new();
    for seed in 0..CASES {
        let mut dice = Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let parent = new_id(now());
        let start = match dice.below(20) {
            0 => Start::Child,
            1 => Start::Forked,
            2 => Start::NotCreated,
            _ => Start::Main,
        };
        let mut make = Make::new(&parent, first_line(&created, start), policy.clone());
        let steps = 10 + usize::try_from(dice.below(400)).expect("测试里造得出");
        make.run(&mut dice, steps);
        let mut lines = make.lines;
        // 四份里有一份停在一步刚派出调用的地方（父会话这一轮正派着 fork 子代理）。
        let calling: Vec<usize> = (0..lines.len())
            .filter(|k| lines[*k].contains(r#""tool_call""#))
            .collect();
        if dice.below(4) == 0
            && let Some(k) = dice.pick(&calling)
        {
            lines.truncate(k + 1);
        }
        let pending = pending_of(&lines);
        let shape = spoil(&mut lines, &mut dice);
        let dir = home.root.session_dir(&parent);
        if shape != "没有这个会话" {
            write(&dir, &lines, &mut dice);
        }
        let call = match dice.below(3) {
            0 if !pending.is_empty() => dice.pick(&pending).copied(),
            1 => CallId::new(Seq::new(7).expect("测试里造得出"), 9),
            _ => None,
        };
        let (old_child, new_child) = (new_id(now()), new_id(now()));
        let at = now();
        let old = old::fork(setup(&home, &parent, (old_child.clone(), at), call, seed));
        let new =
            miyu_session::fork(setup(&home, &parent, (new_child.clone(), at), call, seed)).await;
        assert_eq!(
            told(&new),
            told(&old),
            "种子 {seed}（{shape}，{} 行，{call:?}）",
            lines.len()
        );
        if old.is_ok() {
            assert_eq!(
                written(&home, &new_child),
                written(&home, &old_child),
                "种子 {seed}：写下的第一批逐字节一样"
            );
        }
        let kind = told(&old)
            .split(['(', ' '])
            .next()
            .unwrap_or_default()
            .to_string();
        *outcomes
            .entry(format!(
                "{kind}{}",
                if call.is_some() {
                    "（fork 子代理）"
                } else {
                    ""
                }
            ))
            .or_default() += 1;
    }
    for kind in [
        "Ok",
        "Ok（fork 子代理）",
        "Err::Log",
        "Err::NotMain",
        "Err::Kernel",
        "Err::Kernel（fork 子代理）",
        "Err::Blob",
    ] {
        assert!(
            outcomes.contains_key(kind),
            "一次都没走到「{kind}」：{outcomes:?}"
        );
    }
}

/// 第一条：照真造的会话那一条改。
fn first_line(created: &Event, start: Start) -> String {
    let mut event = created.clone();
    let Body::SessionCreated(body) = &mut event.body else {
        panic!("第一条是造会话");
    };
    let other = new_id(now());
    match start {
        Start::Main => {}
        Start::Child => {
            body.parent = Some(other);
            body.depth = Some(1);
        }
        Start::Forked => {
            body.parent = Some(other.clone());
            body.forked = Some(Forked {
                from: other,
                upto: Seq::new(1).expect("测试里造得出"),
                seal_after: None,
                within: None,
            });
        }
        Start::NotCreated => {
            return r#"{"seq":1,"at":"2026-11-03T08:00:01.000Z","kind":"message.user","by":{"kind":"person"},"body":{"blocks":[]}}"#.to_string();
        }
    }
    event.to_line()
}

/// 照这几行过账本，最后还在等结果的调用。
fn pending_of(lines: &[String]) -> Vec<CallId> {
    let mut ledger = miyu_kernel::ledger::Ledger::for_session(new_id(now()));
    for line in lines {
        let Ok(event) = Event::from_line(line) else {
            break;
        };
        if ledger.append(&event).is_err() {
            break;
        }
    }
    ledger.pending_calls()
}

/// 弄坏几种（多半不坏），交回是哪一种。
fn spoil(lines: &mut Vec<String>, dice: &mut Dice) -> &'static str {
    let at = usize::try_from(dice.below(lines.len() as u64)).expect("测试里造得出");
    match dice.below(16) {
        0 => {
            lines[at] = format!(r#"{{"seq":{},"#, at + 1);
            "坏了一行"
        }
        1 => {
            let last = lines.len() - 1;
            lines[last] = format!(r#"{{"seq":{},"kind":"#, last + 1);
            "最后一行坏了"
        }
        2 => "末尾半行",
        3 => {
            lines.clear();
            "一条都没有"
        }
        4 => "没有这个会话",
        5 | 6 if at > 0 => {
            // 序号照旧连着、读得出，过不了账本：写成不在跑的回合的一句。
            lines[at] = astray(at + 1);
            let later = at
                + 1
                + usize::try_from(dice.below((lines.len() - at) as u64)).expect("测试里造得出");
            if dice.coin() && later < lines.len() {
                lines[later] = format!(r#"{{"seq":{},"#, later + 1);
                return "过不了账本，后面又坏了一行";
            }
            "过不了账本"
        }
        7 => {
            // 一轮的结束换成这一轮里排着的一句：下一轮开不了。
            let ended: Vec<usize> = (1..lines.len())
                .filter(|k| lines[*k].contains(r#""kind":"turn.ended""#))
                .collect();
            if let Some(&k) = dice.pick(&ended) {
                let turn = Event::from_line(&lines[k])
                    .expect("测试里造得出")
                    .turn
                    .expect("测试里造得出");
                lines[k] = format!(
                    r#"{{"seq":{},"at":"2026-11-03T08:00:00.000Z","kind":"message.user","turn":{},"by":{{"kind":"person"}},"body":{{"blocks":[]}}}}"#,
                    k + 1,
                    turn.started()
                );
            }
            "一轮没结束"
        }
        _ => "好的",
    }
}

/// 第 `seq` 条：不在跑的回合里的一句回复。
fn astray(seq: usize) -> String {
    format!(
        r#"{{"seq":{seq},"at":"2026-11-03T08:00:00.000Z","kind":"message.assistant","turn":99999,"by":{{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}},"body":{{"blocks":[],"seen":1}}}}"#
    )
}

/// 照先后分几段写进 `dir`（段名是这一段第一条的序号）；末尾半行的那一份多写半行。
fn write(dir: &Path, lines: &[String], dice: &mut Dice) {
    fs::create_dir_all(dir).expect("测试里造得出");
    let mut cuts: Vec<usize> = (0..dice.below(3))
        .map(|_| usize::try_from(dice.below(lines.len() as u64 + 1)).expect("测试里造得出"))
        .collect();
    cuts.push(0);
    cuts.sort_unstable();
    cuts.dedup();
    for (k, start) in cuts.iter().enumerate() {
        let end = cuts.get(k + 1).copied().unwrap_or(lines.len());
        let mut text: String = lines[*start..end]
            .iter()
            .map(|line| format!("{line}\n"))
            .collect();
        if end == lines.len() && dice.below(8) == 0 {
            text.push_str(r#"{"seq":"#);
        }
        fs::write(dir.join(format!("{:012}.jsonl", start + 1)), text).expect("测试里造得出");
    }
}

/// 分叉要的几样：子会话 `child`、此刻 `at`，fork 子代理的派它的调用 `call`。
fn setup<'a>(
    home: &'a Home,
    parent: &'a SessionId,
    (child, at): (SessionId, miyu_kernel::time::Timestamp),
    call: Option<CallId>,
    seed: u64,
) -> Fork<'a> {
    Fork {
        root: &home.root,
        parent,
        id: child,
        cause: CommandId::parse("f1").expect("测试里造得出"),
        by: alice(),
        at,
        index: None,
        seal_after: std::num::NonZeroU32::new(u32::try_from(seed % 3).expect("测试里造得出")),
        within: call.map(|call| Within {
            call,
            depth: 2,
            placeholder: "Forked; the parent keeps going.".to_string(),
        }),
    }
}

/// 交回的写出来：分叉点，或者报的错（哪一种、写的什么）。
fn told(result: &Result<Seq, ForkError>) -> String {
    match result {
        Ok(upto) => format!("Ok({upto})"),
        Err(error) => format!("Err::{error:?}"),
    }
}

/// 子会话 `child` 写下的每一行。
fn written(home: &Home, child: &SessionId) -> Vec<String> {
    home.log(child).iter().map(Event::to_line).collect()
}
