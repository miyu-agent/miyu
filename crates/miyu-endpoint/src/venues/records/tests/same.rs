//! 边读边记和以前整份读一个字节不差（施工 V-39 上）：照种子造三千段群会话（`script.rs`，分六份并着跑），每一条都当要判的
//! 那一条，几样 `count`、`grow`，放的时机、一块多大也照种子变，新旧比 `Ok(records, current, count)` 或拒绝码。撤销、撤回的
//! 下界另查：它以前的那一截，这一次撤销、撤回前后在有效历史里原样都在。三条退路（再读一遍、整份读、读回引用的那一条）都要
//! 走到。
//!
//! 施工 V-39 上补：新的那一份喂的是每一行只解判官要的种类读成的（`Event::from_line_only`，同会话目录里照
//! `read_range_only` 读的），准绳照旧是全解的。

use miyu_assemble::GroupChat;
use miyu_assemble::group::{Window, records_in};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_store::log::{Flow, OpenError};

use super::old;
use super::rendering as chat;
use super::script::{Dice, Script};
use crate::refusal::Refusal;
use crate::venues::records::bones::Bones;
use crate::venues::records::pass::Tuning;
use crate::venues::records::read::{Log, ONLY, judge};

/// 内存里的日志：一块交 `chunk` 条；数着各种读法用了几次。
pub(super) struct Mem {
    pub(super) events: Vec<Event>,
    pub(super) chunk: usize,
    /// 第一次照 `range` 读完以后接着写下的（读的时候日志还在长）。
    pub(super) grows: Vec<Event>,
    /// 从头读了几遍、整份读了几遍、照序号读回一条几次。
    pub(super) reads: [usize; 3],
}

impl Mem {
    pub(super) fn new(events: Vec<Event>, chunk: usize) -> Mem {
        Mem {
            events,
            chunk: chunk.max(1),
            grows: Vec::new(),
            reads: [0; 3],
        }
    }
}

impl Log for Mem {
    fn range(
        &mut self,
        from: Seq,
        upto: Option<Seq>,
        each: &mut dyn FnMut(Vec<Event>) -> Flow,
    ) -> Result<(), OpenError> {
        match (from, upto) {
            (Seq::FIRST, _) => self.reads[0] += 1,
            (from, Some(upto)) if from == upto => self.reads[2] += 1,
            _ => {}
        }
        let start = self.events.partition_point(|event| event.seq < from);
        let end = upto.map_or(self.events.len(), |upto| {
            self.events.partition_point(|event| event.seq <= upto)
        });
        for block in self.events[start..end.max(start)].chunks(self.chunk) {
            if matches!(each(block.to_vec()), Flow::Stop) {
                break;
            }
        }
        self.events.append(&mut self.grows);
        Ok(())
    }

    fn whole(&mut self) -> Result<Vec<Event>, OpenError> {
        self.reads[1] += 1;
        Ok(self.events.clone())
    }
}

/// 窗口：判官出厂的、`count` 最多的、步长各样。
pub(super) const WINDOWS: [Window; 6] = [
    Window {
        count: 20,
        grow: Some(40),
    },
    Window {
        count: 1,
        grow: None,
    },
    Window {
        count: 20,
        grow: None,
    },
    Window {
        count: 100,
        grow: None,
    },
    Window {
        count: 3,
        grow: Some(2),
    },
    Window {
        count: 1,
        grow: Some(99),
    },
];

/// 照种子的一段日志的每一行。
pub(super) fn lines(seed: u64) -> Vec<String> {
    let mut dice = Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let steps = 20 + dice.below(110);
    Script::run(&mut dice, steps).lines
}

/// 照种子的一段日志：每一行全解。
pub(super) fn events(seed: u64) -> Vec<Event> {
    lines(seed)
        .iter()
        .map(|line| Event::from_line(line).expect("合写法"))
        .collect()
}

/// 同一段日志，每一行只解判官要的种类（施工 V-39 上补）。
pub(super) fn only(seed: u64) -> Vec<Event> {
    lines(seed)
        .iter()
        .map(|line| Event::from_line_only(line, ONLY).expect("合写法"))
        .collect()
}

/// 照种子的放法：多半放得勤、留得少（逼出放早了的），偶尔照出厂的。
fn tuning(seed: u64) -> Tuning {
    let mut dice = Dice(seed ^ 0x5DEE_CE66_D1CE_4E5B);
    if seed.is_multiple_of(7) {
        return Tuning::SHIPPED;
    }
    Tuning {
        flush_at: 1 + dice.below(24) as usize,
        keep: 1 + dice.below(10) as usize,
    }
}

/// 要判的：每一条人话，加第一条、最后一条、最后一条以后、撤销那一条。
fn judged(events: &[Event]) -> Vec<Seq> {
    let mut all: Vec<Seq> = events
        .iter()
        .filter(|event| {
            matches!(
                event.body,
                Body::MessageUser(_)
                    | Body::TurnReverted(_)
                    | Body::MessageWithdrawn(_)
                    | Body::ToolResult(_)
            )
        })
        .map(|event| event.seq)
        .collect();
    let last = events.last().map_or(Seq::FIRST, |event| event.seq);
    all.extend([Seq::FIRST, last, last.next()]);
    all
}

/// 撤销、撤回的下界：它以前的那一截，这一次前后在有效历史里原样都在。交回查了几次。
fn reaches_hold(events: &[Event]) -> usize {
    let mut bones = Bones::default();
    let mut history = History::whole();
    let mut checked = 0;
    for event in events {
        if let Ok(Some(reach)) = bones.reach(event) {
            let before: Vec<Event> = below(&history, reach);
            let mut after = history.clone();
            after.append(event.clone());
            assert_eq!(
                below(&after, reach),
                before,
                "第 {} 条的下界 {reach}",
                event.seq
            );
            checked += 1;
        }
        bones.note(event);
        history.append(event.clone());
    }
    checked
}

/// 只解要的种类的保证（施工 V-39 上补）：一种都不要（只有内核里有效历史认的那几种全解、回复和请求只解几格），交给留着一切的
/// 有效历史，每一次撤销、恢复、撤回、开新的一轮以后留下的（序号、种类）和全解的一样，最后也一样。
fn history_holds(seed: u64, events: &[Event]) {
    let bare = lines(seed)
        .into_iter()
        .map(|line| Event::from_line_only(&line, &[]).expect("合写法"));
    let shape = |history: &History| {
        history
            .events()
            .iter()
            .map(|event| (event.seq, event.body.kind().to_string()))
            .collect::<Vec<_>>()
    };
    let mut full = History::whole();
    let mut skim = History::whole();
    for (event, bare) in events.iter().zip(bare) {
        full.append(event.clone());
        skim.append(bare);
        if matches!(
            event.body,
            Body::TurnReverted(_)
                | Body::TurnUnreverted(_)
                | Body::MessageWithdrawn(_)
                | Body::TurnStarted(_)
        ) {
            assert_eq!(
                shape(&skim),
                shape(&full),
                "种子 {seed} 第 {} 条",
                event.seq
            );
        }
    }
    assert_eq!(shape(&skim), shape(&full), "种子 {seed}");
}

fn below(history: &History, reach: Seq) -> Vec<Event> {
    history
        .events()
        .iter()
        .take_while(|event| event.seq < reach)
        .cloned()
        .collect()
}

/// 第 `k` 条要判的比不比第 `w` 样窗口：都比的太慢（调试版三千段要十分钟），平时比出厂的那样和轮着的两样；
/// `MIYU_RECORDS_FULL=1` 的每样都比（施工时照它跑过一遍 release）。
fn every(k: usize, w: usize) -> bool {
    w == 0 || (k + w).is_multiple_of(3) || std::env::var_os("MIYU_RECORDS_FULL").is_some()
}

/// 第 `seeds` 段：新旧比，交回三条退路各走了几次。
fn compare(seeds: std::ops::RangeInclusive<u64>) -> [usize; 3] {
    let mut paths = [0; 3];
    let texts = |_: Option<&Event>| Ok((chat(), None));
    for seed in seeds {
        let events = events(seed);
        let only = only(seed);
        reaches_hold(&events);
        history_holds(seed, &events);
        let mut history = History::whole();
        for event in events.clone() {
            history.append(event);
        }
        let tuning = tuning(seed);
        for (k, msg) in judged(&events).into_iter().enumerate() {
            for (w, &window) in WINDOWS.iter().enumerate().filter(|(w, _)| every(k, *w)) {
                let mut log = Mem::new(only.clone(), 1 + (seed as usize + k + w) % 9);
                let new = judge(&mut log, msg, window, texts, tuning);
                // 以前的读法照同一份有效历史挑（`old::judge` 每次整份重放，三千段太慢；它和这一样）。
                let old =
                    records_in(&history, msg, window, &chat(), None).ok_or(Refusal::BAD_PARAMS);
                assert_eq!(new, old, "种子 {seed} 第 {msg} 条 {window:?} {tuning:?}");
                paths[0] += usize::from(log.reads[0] > 1);
                paths[1] += log.reads[1];
                paths[2] += log.reads[2];
            }
        }
    }
    paths
}

// 三千段分六份并着跑；三条退路合起来都要走到（整份读的那条靠骨架认不得的几段，种子里约百分之一）。
#[test]
fn chats_read_the_same_as_before_1() {
    let paths = compare(1..=500);
    assert!(paths.iter().all(|&n| n > 0), "{paths:?}");
}

#[test]
fn chats_read_the_same_as_before_2() {
    compare(501..=1000);
}

#[test]
fn chats_read_the_same_as_before_3() {
    compare(1001..=1500);
}

#[test]
fn chats_read_the_same_as_before_4() {
    compare(1501..=2000);
}

#[test]
fn chats_read_the_same_as_before_5() {
    compare(2001..=2500);
}

#[test]
fn chats_read_the_same_as_before_6() {
    compare(2501..=3000);
}

/// 读的时候日志还在往后写：再读、读回引用的那一条都只读头一遍读到的那一截，和以前读到那里为止的一样。
#[test]
fn a_log_growing_while_read_reads_the_first_read() {
    let mut grew = 0;
    for seed in 1..=300 {
        let all = events(seed);
        let cut = all.len() * 2 / 3;
        let head = &all[..cut];
        let only = only(seed);
        for msg in judged(head) {
            let mut log = Mem::new(only[..cut].to_vec(), 3);
            log.grows = only[cut..].to_vec();
            let texts = |_: Option<&Event>| Ok::<(GroupChat, _), _>((chat(), None));
            let tuning = Tuning {
                flush_at: 2,
                keep: 1,
            };
            let new = judge(&mut log, msg, WINDOWS[0], texts, tuning);
            let old = old::judge(Ok(head.to_vec()), msg, WINDOWS[0], texts);
            assert_eq!(new, old, "种子 {seed} 第 {msg} 条");
            grew += usize::from(log.reads[0] > 1 || log.reads[2] > 0);
        }
    }
    assert!(grew > 0, "长了以后要再读的一次都没有");
}

/// 引用照编号的哈希记：撞了的（记的那一条编号不是这个）不拿来用，退回整份读；留着的、读回来的都对一遍。
#[test]
fn a_quote_whose_hash_collides_is_not_used() {
    use crate::venues::records::bones::hash;
    use crate::venues::records::pass::{Pass, Plan};
    use crate::venues::records::read::{Quote, quoted};
    let line = |seq: u64, msg: &str, reply_to: &str| {
        Event::from_line(&format!(
            r#"{{"seq":{seq},"at":"2026-11-03T00:00:00.000Z","kind":"message.user","by":{{"kind":"person"}},"body":{{"blocks":[],"venue":{{"msg":"{msg}"{reply_to}}}}}}}"#
        ))
        .expect("合写法")
    };
    let judged = line(9, "m9", r#","reply_to":"m2""#);
    let mut pass = Pass::new(judged.seq, 20, Plan::exact(Vec::new()), Tuning::SHIPPED);
    pass.ring.push_back(line(5, "m1", ""));
    // 「m2」的哈希记成了第 5 条（编号是 m1）：撞了。
    pass.quotes.insert(hash("m2"), Seq::new(5).expect("合写法"));
    let mut log = Mem::new(vec![line(3, "m1", ""), line(5, "m1", "")], 1);
    assert_eq!(quoted(&mut log, &pass, &judged), Ok(Quote::Lost));
    pass.ring.clear();
    assert_eq!(
        quoted(&mut log, &pass, &judged),
        Ok(Quote::Lost),
        "读回来的也对一遍"
    );
    pass.quotes.insert(hash("m2"), Seq::new(3).expect("合写法"));
    let mut log = Mem::new(vec![line(3, "m2", "")], 1);
    assert!(matches!(
        quoted(&mut log, &pass, &judged),
        Ok(Quote::Read(_))
    ));
}
