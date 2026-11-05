//! 新旧对照（施工 V-41 中）：撤销、恢复的回应（[`report`]）和以前整份读、整份过账本的（照抄在 `old.rs`）逐字一样。照种子造的
//! 会话（`miyu_session::testkit::undo_log`，过得了账本）写成几段真文件，日志里每一次撤销、恢复都写一份回应，新旧比 JSON。
//! 分叉出来的（第一条过不了从第 1 条起的账本，停掉的任务一律是空的）、撤到分叉点以前的、读的时候日志还停在撤销刚落盘那一刻
//! 的也比。坏一行、末尾半行、不是最后一段的末尾半行、少一行的：坏在新的读到的那一截里的（撤的第一轮开头到最后），和以前一样
//! 交回空的；坏在跳过的那一截里的，同以前照没坏的日志写的一样（跳过的不自检，2026-11-03 项目主人定）。
//!
//! 平时 100 个种子（施工 CI-2：原来的 1000 个 Windows 的 debug 要两三分钟，挪到 CI 的 randomized 那一项照 `MIYU_UNDO_SEEDS`
//! 跑）；施工时跑过 10000 个；`MIYU_UNDO_SEEDS` 改个数。

use std::fs;
use std::path::Path;

use miyu_kernel::event::{Body, Event};
use miyu_session::testkit::undo_log::{Dice, UndoLog};

use super::super::{Sources, report};
use super::old;
use crate::test_support::temp_root;

/// 新的回应，写成 JSON。
fn new(dir: &Path, events: &[u64]) -> String {
    let sources = Sources {
        log: dir.to_path_buf(),
    };
    serde_json::to_string(&report(&sources, events)).expect("写得成 JSON")
}

/// 以前的回应，写成 JSON。
fn was(dir: &Path, events: &[u64]) -> String {
    serde_json::to_string(&old::report(dir, events)).expect("写得成 JSON")
}

/// 种子个数。
fn seeds() -> u64 {
    std::env::var("MIYU_UNDO_SEEDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(QUICK)
}

/// 平时几个种子。
const QUICK: u64 = 100;

/// 照种子造一段（偶尔是分叉出来的，交回父会话的那一段），写进 `dir`。
fn made(dice: &mut Dice, dir: &Path) -> (UndoLog, Option<UndoLog>) {
    let steps = 20 + dice.below(400);
    let (log, parent) = if dice.odds(6) {
        let parent_steps = 10 + dice.below(120);
        let parent = UndoLog::run(dice, parent_steps, None);
        let upto = parent.first + parent.lines.len() as u64 - 1;
        let child = UndoLog::run(dice, steps, Some((upto, parent.turns.clone())));
        (child, Some(parent))
    } else {
        (UndoLog::run(dice, steps, None), None)
    };
    assert_eq!(log.ledger(), None, "造的过得了账本");
    UndoLog::write(dir, log.first, &log.lines, dice);
    (log, parent)
}

/// 要写回应的：每一次撤销、恢复（`events` 照撤销那一批，后面跟着几条），和几样不是撤销的（第一条、最后一条以后的、0）。
fn targets(log: &UndoLog, dice: &mut Dice) -> Vec<Vec<u64>> {
    let mut targets: Vec<Vec<u64>> = log
        .undos
        .iter()
        .map(|&seq| match dice.below(3) {
            0 => vec![seq, seq + 1, seq + 2],
            _ => vec![seq],
        })
        .collect();
    let last = log.first + log.lines.len() as u64 - 1;
    targets.push(vec![log.first]);
    targets.push(vec![last]);
    targets.push(vec![last + 1]);
    targets.push(vec![0]);
    targets.push(vec![log.first + dice.below(log.lines.len() as u64)]);
    targets.push(Vec::new());
    targets
}

#[test]
fn undo_reports_are_the_same_as_reading_the_whole_log() {
    let (_root, temp) = temp_root("undo-same");
    let mut reverts = 0;
    let mut stopped = 0;
    let mut said = 0;
    let mut compacted = 0;
    for seed in 0..seeds() {
        let mut dice = Dice(seed * 2_654_435_761 + 17);
        let dir = temp.join(format!("s{seed}"));
        let (log, _) = made(&mut dice, &dir);
        for events in targets(&log, &mut dice) {
            let was = was(&dir, &events);
            assert_eq!(new(&dir, &events), was, "种子 {seed}，{events:?}");
            reverts += usize::from(!was.contains("\"turns\":0"));
            stopped += usize::from(was.contains("\"jobs\""));
            said += usize::from(was.contains("\"said\""));
            compacted += usize::from(was.contains("\"compactions\"") || was.contains("\"clears\""));
        }
        // 读的时候日志还停在撤销刚落盘那一刻（后面再跟几条）。
        if let Some(&seq) = log
            .undos
            .get(dice.below(log.undos.len() as u64 + 1) as usize)
        {
            let keep = (seq - log.first) as usize + 1 + dice.below(3) as usize;
            let cut = temp.join(format!("c{seed}"));
            UndoLog::write(
                &cut,
                log.first,
                &log.lines[..keep.min(log.lines.len())],
                &mut dice,
            );
            assert_eq!(
                new(&cut, &[seq]),
                was(&cut, &[seq]),
                "种子 {seed} 截到 {seq}"
            );
            drop(fs::remove_dir_all(&cut));
        }
        drop(fs::remove_dir_all(&dir));
    }
    // 几样都比到了。
    let seeds = usize::try_from(seeds()).unwrap_or(usize::MAX);
    eprintln!("撤销 {reverts}，停掉任务 {stopped}，带那句话 {said}，带压缩 {compacted}");
    assert!(
        reverts > seeds && stopped > seeds / 20 && said > seeds && compacted > seeds / 10,
        "撤销 {reverts}，停掉任务 {stopped}，带那句话 {said}，带压缩 {compacted}"
    );
}

/// 怎么弄坏一份日志。
#[derive(Debug, Clone, Copy)]
enum Damage {
    /// 第 k 行换成读不成的。
    Broken(usize),
    /// 少了第 k 行（序号断了）。
    Missing(usize),
    /// 最后一段末尾多半行。
    HalfAtEnd,
}

/// 第 `seq` 条（没坏的日志里的）。
fn event_at(log: &UndoLog, seq: u64) -> Option<Event> {
    let k = usize::try_from(seq.checked_sub(log.first)?).ok()?;
    log.lines
        .get(k)
        .and_then(|line| Event::from_line(line).ok())
}

/// 撤销（恢复）那一条撤的第一轮的开头：新的从它读到最后。
fn read_from(log: &UndoLog, seq: u64) -> Option<u64> {
    match event_at(log, seq)?.body {
        Body::TurnReverted(reverted) => reverted.turns.first().map(|turn| turn.started().get()),
        Body::TurnUnreverted(_) => None,
        _ => None,
    }
}

#[test]
fn damage_read_by_the_new_way_is_reported_and_damage_skipped_is_not_checked() {
    let (_root, temp) = temp_root("undo-damage");
    let mut caught = 0;
    let mut skipped = 0;
    for seed in 0..seeds() / 3 {
        let mut dice = Dice(seed * 40_503 + 99);
        let dir = temp.join(format!("d{seed}"));
        let (log, _) = made(&mut dice, &dir);
        drop(fs::remove_dir_all(&dir));
        let Some(&seq) = log.undos.last() else {
            continue;
        };
        let k = dice.below(log.lines.len() as u64) as usize;
        let damage = match dice.below(3) {
            0 => Damage::Broken(k),
            1 if k > 0 => Damage::Missing(k),
            _ => Damage::HalfAtEnd,
        };
        let mut lines = log.lines.clone();
        match damage {
            Damage::Broken(k) => lines[k] = format!(r#"{{"seq":{},"at":"#, log.first + k as u64),
            Damage::Missing(k) => {
                lines.remove(k);
            }
            Damage::HalfAtEnd => {}
        }
        UndoLog::write(&dir, log.first, &lines, &mut dice);
        if let Damage::HalfAtEnd = damage {
            let last = fs::read_dir(&dir)
                .expect("读得了目录")
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .max()
                .expect("有段");
            let mut text = fs::read(&last).expect("读得了");
            text.extend_from_slice(br#"{"seq":99999,"at":"#);
            fs::write(&last, text).expect("写得了");
        }
        let clean = temp.join(format!("k{seed}"));
        UndoLog::write(&clean, log.first, &log.lines, &mut dice);
        let events = [seq];
        let (now, broken, whole) = (new(&dir, &events), was(&dir, &events), was(&clean, &events));
        let at = match damage {
            Damage::Broken(k) | Damage::Missing(k) => Some(log.first + k as u64),
            Damage::HalfAtEnd => None,
        };
        match (at, read_from(&log, seq)) {
            // 坏在新的读到的那一截里（撤的第一轮开头到最后），或者是末尾的半行（两边都跳过）：同以前。
            (None, _) => assert_eq!(now, broken, "种子 {seed} {damage:?}"),
            (Some(at), Some(from)) if at >= from => {
                assert_eq!(now, broken, "种子 {seed} {damage:?}");
                caught += 1;
            }
            // 坏在跳过的那一截里：要么同以前照坏的写的（定位退回从头读、正好读到了），要么同没坏的。
            _ => {
                assert!(
                    now == broken || now == whole,
                    "种子 {seed} {damage:?}：{now} / {broken} / {whole}"
                );
                skipped += usize::from(now != broken);
            }
        }
        drop(fs::remove_dir_all(&dir));
        drop(fs::remove_dir_all(&clean));
    }
    eprintln!("读到的坏了 {caught}，跳过的 {skipped}");
    assert!(
        caught > 0 && skipped > 0,
        "读到的坏了 {caught}，跳过的 {skipped}"
    );
}

/// 分叉出来的会话：停掉的任务一律是空的（第一条过不了从第 1 条起的账本），照旧记一行 `undo report jobs not read`。撤到分叉点
/// 以前的回合的，那几轮的开头不在自己的日志里，没有那句话。
#[test]
fn a_forked_session_lists_no_stopped_jobs() {
    let (_root, temp) = temp_root("undo-forked");
    let mut checked = 0;
    // 种子数的五分之一（施工 CI-2：原来定死 200 份，平时 20 份，CI 的 randomized 那一项 2000 份）。
    let forks = seeds() / 5;
    for seed in 0..forks {
        let mut dice = Dice(seed * 7919 + 3);
        let parent = UndoLog::run(&mut dice, 60, None);
        let upto = parent.first + parent.lines.len() as u64 - 1;
        let child = UndoLog::run(&mut dice, 300, Some((upto, parent.turns.clone())));
        let dir = temp.join(format!("f{seed}"));
        UndoLog::write(&dir, child.first, &child.lines, &mut dice);
        for &seq in &child.undos {
            let now = new(&dir, &[seq]);
            assert_eq!(now, was(&dir, &[seq]), "种子 {seed}，{seq}");
            assert!(!now.contains("\"jobs\""), "{now}");
            checked += 1;
        }
        drop(fs::remove_dir_all(&dir));
    }
    assert!(checked as u64 > forks, "比了 {checked} 次");
}
