//! 记录窗和整份读一个字节不差（施工 V-44）：照种子造群会话（`script.rs`，一半是长段不撤的），像 actor 那样一批批写进日志、
//! 交给记录窗，中间随时问判官（最新一条、往前几条、很早的、没有的、窗口各样），每一问照端点的 [`answer`]（照窗答，答不了的
//! 整份读、顺手建窗）答，和整份交给留着一切的有效历史挑的（`records_in`）逐字比。窗留多少多半调小，逼出挤掉、不够、撤回
//! 多过上限的几种；另造建窗时日志还在长（读完了 actor 又交了几条、还没交到读到的那一条）的两种。照窗答的、各条退路都要
//! 走到。
//!
//! 平时跑 [`SEEDS`] 段（调试版几秒）；`MIYU_RECORDS_FULL=1` 的跑 [`SEEDS_FULL`] 段。

use miyu_assemble::group::{Window, records_in};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_session::Onlooker;

use super::rendering as chat;
use miyu_store::log::{Flow, OpenError};

use super::same::WINDOWS;
use super::script::{Dice, Script};
use crate::refusal::Refusal;
use crate::venues::records::answer;
use crate::venues::records::read::{Log, ONLY};
use crate::venues::records::recent::{Caps, Recent};

/// 平时跑几段（施工 CI-2：原来 250 段，Windows 的 debug 要二十来秒；全量挪到 CI 的 randomized 那一项）。
const SEEDS: u64 = 80;
/// `MIYU_RECORDS_FULL=1` 跑几段。
const SEEDS_FULL: u64 = 6000;

/// 日志：`all` 的前 `upto` 条，头一回照 `range` 读完以后长到 `grown` 条（读的时候日志还在长）；一块交 `chunk` 条，数着
/// 从头读了几遍、整份读了几遍、照序号读回一条几次（同 `same.rs` 的 `Mem`，只是不拷整份）。
struct Mem<'a> {
    all: &'a [Event],
    upto: usize,
    grown: usize,
    chunk: usize,
    reads: [usize; 3],
}

impl<'a> Mem<'a> {
    fn new(all: &'a [Event], upto: usize, chunk: usize) -> Mem<'a> {
        Mem {
            all,
            upto,
            grown: upto,
            chunk: chunk.max(1),
            reads: [0; 3],
        }
    }
}

impl Log for Mem<'_> {
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
        let events = &self.all[..self.upto];
        let start = events.partition_point(|event| event.seq < from);
        let end = upto.map_or(events.len(), |upto| {
            events.partition_point(|event| event.seq <= upto)
        });
        for block in events[start..end.max(start)].chunks(self.chunk) {
            if matches!(each(block.to_vec()), Flow::Stop) {
                break;
            }
        }
        self.upto = self.grown;
        Ok(())
    }

    fn whole(&mut self) -> Result<Vec<Event>, OpenError> {
        self.reads[1] += 1;
        Ok(self.all[..self.upto].to_vec())
    }
}

/// 照种子的一段：全解的（actor 交的）、只解判官要的（日志里读出来的）。一半撤得少、长一些。
fn session(seed: u64) -> (Vec<Event>, Vec<Event>) {
    let mut dice = Dice(seed.wrapping_mul(0xA24B_AED4_963E_E407) | 1);
    let calm = if seed.is_multiple_of(2) {
        0
    } else {
        4 + dice.below(40)
    };
    let steps = if calm == 0 {
        30 + dice.below(150)
    } else {
        100 + dice.below(400)
    };
    let lines = Script::run_calm(&mut dice, steps, calm).lines;
    let full = lines
        .iter()
        .map(|line| Event::from_line(line).expect("合写法"))
        .collect();
    let only = lines
        .iter()
        .map(|line| Event::from_line_only(line, ONLY).expect("合写法"))
        .collect();
    (full, only)
}

/// 照种子的留法：多半很小，偶尔照出厂的。
fn caps(dice: &mut Dice) -> Caps {
    if dice.odds(5) {
        return Caps::SHIPPED;
    }
    Caps {
        ring: 1 + dice.below(40) as usize,
        quotes: 1 + dice.below(10) as usize,
        recalls: 1 + dice.below(8) as usize,
    }
}

/// 照窗答的、各种退路各几次。
#[derive(Debug, Default)]
struct Seen {
    /// 照窗答的。
    window: usize,
    /// 照窗答、引用的那一条照序号读回的。
    read_back: usize,
    /// 建了窗的（整份读那一问）。
    built: usize,
    /// 窗好好的、答不了退回整份读的。
    missed: usize,
    /// 窗作废了的（撤销、恢复、撤回、撤回多过上限）。
    broken: usize,
    /// 建的时候日志还在长、补读了那一截的。
    caught_up: usize,
    /// 建的时候 actor 还没交到读到的那一条的。
    ahead: usize,
}

/// 要问的：最新的一条人话、往前几条、很早的一条、一条不是人话的、还没有的。
fn pick(dice: &mut Dice, events: &[Event]) -> Seq {
    let said: Vec<Seq> = events
        .iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.seq)
        .collect();
    let last = events.last().map_or(Seq::FIRST, |event| event.seq);
    if said.is_empty() || dice.odds(15) {
        return if dice.odds(2) { last.next() } else { last };
    }
    let back = match dice.below(6) {
        0 => dice.below(said.len() as u64) as usize,
        1 | 2 => dice.below(said.len().min(8) as u64) as usize,
        _ => 0,
    };
    said[said.len() - 1 - back]
}

/// 整份交给留着一切的有效历史挑的。
fn truth(
    history: &History,
    msg: Seq,
    window: Window,
) -> Result<miyu_assemble::group::Records, Refusal> {
    records_in(history, msg, window, &chat(), None).ok_or(Refusal::BAD_PARAMS)
}

/// 第 `seed` 段：一批批写、交，中间问；交回各条路走了几次。
fn replay(seed: u64, seen: &mut Seen) {
    let (full, only) = session(seed);
    let mut dice = Dice(seed ^ 0x2545_F491_4F6C_DD1D);
    let recent = Recent::new(caps(&mut dice));
    let texts = |_: Option<&Event>| Ok((chat(), None));
    // 写进日志的到第几条、交给记录窗的到第几条；有效历史照写进日志的。
    let (mut disk, mut told) = (0, 0);
    let mut history = History::whole();
    while disk < full.len() {
        let n = (1 + dice.below(4) as usize).min(full.len() - disk);
        for event in &full[disk..disk + n] {
            history.append(event.clone());
        }
        let broke = full[disk..disk + n].iter().any(|event| {
            matches!(
                event.body,
                Body::TurnReverted(_) | Body::TurnUnreverted(_) | Body::MessageWithdrawn(_)
            )
        });
        disk += n;
        // 窗还没建好的，偶尔先不交（建窗读到的比交过的新）。
        if recent.built() || !dice.odds(3) {
            let had = recent.built();
            recent.appended(&full[told..disk]);
            told = disk;
            seen.broken += usize::from(had && broke && !recent.built());
        }
        if !dice.odds(3) {
            continue;
        }
        let msg = pick(&mut dice, &full[..disk]);
        let window = WINDOWS[dice.below(WINDOWS.len() as u64) as usize];
        let chunk = 1 + dice.below(7) as usize;
        let had = recent.built();
        if told < disk {
            // 建窗读到的比交过的新：这一问照整份读答（窗还没有），交过来的旧的照序号跳过。
            let mut log = Mem::new(&only, disk, chunk);
            let got = answer(&mut log, Some(&recent), msg, window, texts);
            assert_eq!(
                got,
                truth(&history, msg, window),
                "种子 {seed} 第 {msg} 条（交得慢）"
            );
            let built = recent.built();
            seen.ahead += usize::from(built);
            recent.appended(&full[told..disk]);
            told = disk;
            // 建窗时读过的，交过来照序号跳过，窗照旧好好的。
            assert_eq!(recent.built(), built, "种子 {seed}：交过来的旧的没跳过");
            continue;
        }
        let grow = (dice.odds(4) && !had && disk < full.len())
            .then(|| (1 + dice.below(5) as usize).min(full.len() - disk));
        let mut log = Mem::new(&only, disk, chunk);
        let answers = recent.answers(msg, window);
        if let Some(grow) = grow {
            // 建窗那一遍读完以后又写进来、交过来的几条：装上之前照日志补读。
            log.grown = disk + grow;
            recent.appended(&full[disk..disk + grow]);
        }
        let got = answer(&mut log, Some(&recent), msg, window, texts);
        assert_eq!(
            got,
            truth(&history, msg, window),
            "种子 {seed} 第 {msg} 条 {window:?}"
        );
        if let Some(grow) = grow {
            for event in &full[disk..disk + grow] {
                history.append(event.clone());
            }
            disk += grow;
            told = disk;
            if recent.built() {
                // 装上之前照日志补读到 actor 交过来的最后一条。
                assert_eq!(recent.upto(), Seq::new(disk as u64), "种子 {seed}：没补读");
                seen.caught_up += 1;
            }
        }
        match (had, answers) {
            (true, true) => {
                seen.window += 1;
                seen.read_back += log.reads[2];
                assert_eq!(log.reads[0] + log.reads[1], 0, "照窗答的不读日志");
            }
            (true, false) => seen.missed += 1,
            (false, _) => seen.built += usize::from(recent.built()),
        }
    }
    // 写完了：再问几条（最新的、往前的、很早的）。
    for _ in 0..6 {
        let msg = pick(&mut dice, &full);
        let window = WINDOWS[dice.below(WINDOWS.len() as u64) as usize];
        let mut log = Mem::new(&only, only.len(), 4);
        let got = answer(&mut log, Some(&recent), msg, window, texts);
        assert_eq!(
            got,
            truth(&history, msg, window),
            "种子 {seed} 末了第 {msg} 条"
        );
    }
}

fn seeds() -> u64 {
    if std::env::var_os("MIYU_RECORDS_FULL").is_some() {
        SEEDS_FULL
    } else {
        SEEDS
    }
}

#[test]
fn windows_answer_the_same_as_whole_reads() {
    let mut seen = Seen::default();
    for seed in 1..=seeds() {
        replay(seed, &mut seen);
    }
    eprintln!("{seen:?}");
    for (what, n) in [
        ("照窗答", seen.window),
        ("引用读回", seen.read_back),
        ("建窗", seen.built),
        ("答不了", seen.missed),
        ("作废", seen.broken),
        ("补读", seen.caught_up),
        ("交得慢", seen.ahead),
    ] {
        assert!(n > 0, "{what} 一次都没有：{seen:?}");
    }
}

/// 没载入的会话（没有窗）照旧整份读；有窗的头一问建窗，第二问照窗答、不读日志。
#[test]
fn the_first_ask_builds_and_the_next_reads_the_window() {
    let (full, only) = (1..200)
        .map(session)
        .find(|(full, _)| {
            full.len() > 60
                && !full.iter().any(|event| {
                    matches!(
                        event.body,
                        Body::TurnReverted(_) | Body::TurnUnreverted(_) | Body::MessageWithdrawn(_)
                    )
                })
        })
        .expect("有一段不撤的");
    let mut history = History::whole();
    for event in &full {
        history.append(event.clone());
    }
    let msg = full
        .iter()
        .rev()
        .find(|event| {
            miyu_assemble::group::on_record(event) && matches!(event.body, Body::MessageUser(_))
        })
        .expect("有人话")
        .seq;
    let texts = |_: Option<&Event>| Ok((chat(), None));
    let mut log = Mem::new(&only, only.len(), 5);
    assert_eq!(
        answer(&mut log, None, msg, WINDOWS[0], texts),
        truth(&history, msg, WINDOWS[0])
    );
    let recent = Recent::new(Caps::SHIPPED);
    recent.appended(&full);
    let mut log = Mem::new(&only, only.len(), 5);
    assert_eq!(
        answer(&mut log, Some(&recent), msg, WINDOWS[0], texts),
        truth(&history, msg, WINDOWS[0])
    );
    assert!(recent.built() && log.reads[0] > 0);
    let mut log = Mem::new(&only, only.len(), 5);
    assert_eq!(
        answer(&mut log, Some(&recent), msg, WINDOWS[0], texts),
        truth(&history, msg, WINDOWS[0])
    );
    assert_eq!(log.reads, [0, 0, 0], "第二问不读日志");
}

/// 引用照编号的哈希记：撞了的（记的那一条编号不是这个）不拿来用，退回整份读；在窗里的、读回来的都对一遍。
#[test]
fn a_window_quote_whose_hash_collides_is_not_used() {
    let line = |seq: u64, msg: &str, reply_to: &str| {
        let kind = if seq == 1 {
            "session.created"
        } else {
            "message.user"
        };
        let body = if seq == 1 {
            r#"{"venue":"qq:group:1","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"full","read_only":false}}"#.to_string()
        } else {
            format!(
                r#"{{"blocks":[{{"type":"text","text":"早"}}],"venue":{{"msg":"{msg}"{reply_to}}}}}"#
            )
        };
        Event::from_line(&format!(
            r#"{{"seq":{seq},"at":"2026-11-03T00:00:00.000Z","kind":"{kind}","by":{{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}},"body":{body}}}"#
        ))
        .expect("合写法")
    };
    let seq = |n: u64| Seq::new(n).expect("合写法");
    let events = vec![
        line(1, "", ""),
        line(2, "m1", ""),
        line(3, "m2", ""),
        line(4, "m3", ""),
        line(5, "m4", r#","reply_to":"m3""#),
    ];
    let mut history = History::whole();
    for event in &events {
        history.append(event.clone());
    }
    let texts = |_: Option<&Event>| Ok((chat(), None));
    let window = Window {
        count: 1,
        grow: None,
    };
    for (caps, at, what) in [(20, 3, "窗里的"), (2, 3, "读回的")] {
        let recent = Recent::new(Caps {
            ring: caps,
            ..Caps::SHIPPED
        });
        recent.appended(&events);
        let mut log = Mem::new(&events, events.len(), 2);
        assert_eq!(
            answer(&mut log, Some(&recent), seq(5), window, texts),
            truth(&history, seq(5), window)
        );
        assert!(recent.built());
        // 「m3」的哈希记成了第 `at` 条（编号不是 m3）：撞了。
        recent.misquote("m3", seq(at));
        let mut log = Mem::new(&events, events.len(), 2);
        assert_eq!(
            answer(&mut log, Some(&recent), seq(5), window, texts),
            truth(&history, seq(5), window),
            "{what}"
        );
        assert!(log.reads[0] > 0, "{what}：撞了的退回整份读");
        assert_eq!(
            log.reads[2] > 0,
            caps == 2,
            "{what}：不在窗里的先照序号读回"
        );
    }
}
