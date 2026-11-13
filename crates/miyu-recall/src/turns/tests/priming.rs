//! 一条条铺（施工 V-37 下，[`Priming`]、[`Replay`]）：和原来整份铺的（照抄留在这里的 [`old_primed_on`]、[`old_replay`]）比，
//! 交回的 `TurnFeed`（`Debug` 写出来的每一样）、改动逐个相等。日志照种子造：本机的人、平台上的人、不是人的、人生循环和
//! 别的循环叫醒的（施工 LF-3）说话，照一句开、照几句开、不带触发的回合，并进来的，有字没字的回复，几种结束，撤销、恢复，
//! 不在这一轮的回复；底子切在各处，照到哪落在各处。

use std::collections::BTreeMap;

use super::*;

const MODULE: &str = r#"{"kind":"module","id":"goal"}"#;
/// 人生循环叫醒的（施工 LF-3）：进回合索引，字只有她的回复；她自己定节奏的循环叫醒的不进。
const LIFE: &str = r#"{"kind":"schedule","schedule":"t1","loop":"life"}"#;
const DYNAMIC: &str = r#"{"kind":"schedule","schedule":"t2","loop":"dynamic"}"#;

/// 原来的 `TurnFeed::primed_on`（施工 R-13 三补起，V-37 下以前），照抄。
fn old_primed_on(base: &[Event], events: &[Event], after: Option<Seq>) -> (TurnFeed, Vec<Change>) {
    let mut feed = TurnFeed::default();
    for event in base {
        feed.see(event);
    }
    feed.ended = Some(BTreeMap::new());
    let mut changes = Vec::new();
    for event in events {
        let seen = feed.see(event);
        if after.is_none_or(|after| event.seq > after) {
            changes.extend(seen);
        }
    }
    feed.ended = None;
    (feed, changes)
}

/// 原来的 `replay`（施工 R-2 上起，V-37 下以前），照抄。
fn old_replay(events: &[Event]) -> Vec<TurnItem> {
    let (_, changes) = old_primed_on(&[], events, None);
    let mut kept = BTreeMap::new();
    for change in changes {
        match change {
            Change::Put(item) => {
                kept.insert(item.turn, item);
            }
            Change::Remove(turn) => {
                kept.remove(&turn);
            }
            Change::Restored(_) | Change::Lost(_) => {}
        }
    }
    kept.into_values().collect()
}

/// 照种子走的随机数（xorshift）。
struct Dice(u64);

impl Dice {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }
}

/// 一份日志，一到两百来条。
fn log(dice: &mut Dice) -> Vec<Event> {
    let mut events = Vec::new();
    let mut pending: Vec<u64> = Vec::new();
    let mut open: Option<u64> = None;
    let mut ended: Vec<u64> = Vec::new();
    let mut reverted: Vec<Vec<u64>> = Vec::new();
    let words = ["早", "", "周末去爬山", "团子吃什么", "好"];
    let target = 1 + dice.below(200);
    while (events.len() as u64) < target {
        let seq = events.len() as u64 + 1;
        let pick = |dice: &mut Dice, from: &[u64]| -> Vec<u64> {
            from.iter()
                .copied()
                .filter(|_| dice.below(2) == 0)
                .collect()
        };
        let event = match dice.below(12) {
            0..=2 => {
                let by = [ALICE, BOB, CAROL, MODULE, LIFE, DYNAMIC][dice.below(6) as usize];
                pending.push(seq);
                Some(said(seq, by, words[dice.below(5) as usize]))
            }
            3 | 4 if open.is_none() => {
                open = Some(seq);
                let event = match (dice.below(3), pending.last()) {
                    (0, _) | (_, None) => started(seq, None),
                    (1, Some(_)) => {
                        let picked = pick(dice, &pending);
                        match picked.is_empty() {
                            true => started(seq, None),
                            false => responded(seq, &picked),
                        }
                    }
                    (_, Some(_)) => {
                        let at = dice.below(pending.len() as u64) as usize;
                        started(seq, Some(pending[at]))
                    }
                };
                if dice.below(4) > 0 {
                    pending.clear();
                }
                Some(event)
            }
            5 => open.and_then(|turn| {
                let picked = pick(dice, &pending);
                (!picked.is_empty()).then(|| joined(seq, turn, &picked))
            }),
            6 | 7 => open.map(|turn| replied(seq, turn, words[dice.below(5) as usize])),
            8 => open.take().map(|turn| {
                ended.push(turn);
                ended_line(
                    seq,
                    turn,
                    ["completed", "interrupted", "error"][dice.below(3) as usize],
                )
            }),
            9 if open.is_none() && !ended.is_empty() => {
                let from = ended.len().saturating_sub(1 + dice.below(2) as usize);
                let turns = ended.split_off(from);
                let event = reverted_line(seq, &turns);
                reverted.push(turns);
                Some(event)
            }
            10 if open.is_none() => reverted.pop().map(|turns| {
                ended.extend(turns.iter().copied());
                unreverted_line(seq, &turns)
            }),
            11 => ended.last().map(|turn| replied(seq, *turn, "晚了的一句")),
            _ => None,
        };
        events.extend(event);
    }
    events
}

fn ended_line(seq: u64, turn: u64, reason: &str) -> Event {
    ended(seq, turn, reason)
}

fn reverted_line(seq: u64, turns: &[u64]) -> Event {
    reverted(seq, turns)
}

fn unreverted_line(seq: u64, turns: &[u64]) -> Event {
    unreverted(seq, turns)
}

#[test]
fn priming_one_by_one_gives_what_priming_the_whole_did() {
    let mut dice = Dice(0x5eed_2026_1103);
    let (mut restored, mut put_late) = (0, 0);
    for round in 0..3000 {
        let events = log(&mut dice);
        let cut = dice.below(events.len() as u64 + 1) as usize;
        let (base, own) = events.split_at(cut);
        let after = match dice.below(3) {
            0 => None,
            _ => Seq::new(dice.below(events.len() as u64 + 2)),
        };
        let (old_feed, old_changes) = old_primed_on(base, own, after);
        let (feed, changes) = TurnFeed::primed_on(base, own, after);
        assert_eq!(
            format!("{feed:?}"),
            format!("{old_feed:?}"),
            "第 {round} 份"
        );
        assert_eq!(changes, old_changes, "第 {round} 份");
        let mut priming = Priming::default();
        base.iter().for_each(|event| priming.base(event));
        own.iter().for_each(|event| priming.own(event));
        let (feed, changes) = priming.finish(after);
        assert_eq!(
            format!("{feed:?}"),
            format!("{old_feed:?}"),
            "第 {round} 份"
        );
        assert_eq!(changes, old_changes, "第 {round} 份");
        restored += usize::from(
            old_changes
                .iter()
                .any(|change| matches!(change, Change::Restored(_))),
        );
        put_late += usize::from(
            after.is_some()
                && old_changes
                    .iter()
                    .any(|change| matches!(change, Change::Put(_))),
        );

        assert_eq!(replay(&events), old_replay(&events), "第 {round} 份");
        let mut one = Replay::default();
        events.iter().for_each(|event| one.see(event));
        assert_eq!(one.finish(), old_replay(&events), "第 {round} 份");
    }
    assert!(restored > 100, "恢复了的 {restored}");
    assert!(put_late > 100, "照到一半以后还放的 {put_late}");
}
