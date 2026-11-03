//! 撤销、恢复以后重算她看过的文件（施工 V-41 中）：边读边记（[`Store::seen`]）和以前整份读进内存再算的（照抄在 `seen/old.rs`）
//! 一样，报的错一样。照种子造的会话（`crate::testkit::undo_log`）写成几段真文件；日志停在每一次撤销、恢复刚落盘那一刻的（后面
//! 偶尔再跟几条）都比。分叉出来的连同底子（父会话的日志，读到分叉点为止）比。坏一行、少一行、不是最后一段的末尾半行，报的错
//! 和以前一字不差。
//!
//! 和以前不一样的只有两样，都在下面单独的测试里写明：不要的种类的正文合 JSON、不合那一种的写法的不报坏了（只解要的种类，同
//! 判官的，2026-11-03 项目主人拍板接受）；底子在分叉点以后坏了的不报（读到分叉点就停，同施工 V-37 下的 `Base::each`）。
//!
//! 默认 1000 个种子（施工时跑过 10000 个）；`MIYU_UNDO_SEEDS` 改个数。

use std::fs;
use std::path::{Path, PathBuf};

use miyu_kernel::id::{Seq, SessionId};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog, read_events};
use miyu_tool::Seen;

use super::super::{Base, Indexed, Store, scan_own};
use crate::effects::SeenScan;
use crate::testkit::undo_log::{Dice, UndoLog};

mod old;

/// 数据根里的临时目录（会话日志只在数据根里开，施工 TMP-1）：用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let path =
            std::env::temp_dir().join(format!("miyu-session-seen-{name}-{}", std::process::id()));
        drop(fs::remove_dir_all(&path));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join(miyu_store::root::MARKER), "").unwrap();
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// 种子个数。
fn seeds() -> u64 {
    std::env::var("MIYU_UNDO_SEEDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(1000)
}

/// 新旧：以前整份读（`Store::events`）再算的，和边读边记的。
fn both(store: &dyn Store) -> (Result<Seen, String>, Result<Seen, String>) {
    let was = store.events().map(|events| old::seen_in(&events));
    (was, store.seen())
}

/// 打开 `dir` 的日志；分叉出来的接上底子（父会话的日志在 `base`，到第 `upto` 条为止）。
fn open(dir: &Path, base: Option<(&Path, u64)>) -> Box<dyn Store> {
    let (log, _) = SessionLog::open(dir, SEGMENT_LIMIT).expect("打得开");
    match base {
        None => Box::new(log),
        Some((parent, upto)) => {
            let id = SessionId::parse("01a0f234-0a1b-7c2d-8e3f-4a5b6c7d8e9f").unwrap();
            Box::new(
                Indexed::new(log, &id, None, None, None)
                    .on_base(Base::new(parent.to_path_buf(), Seq::new(upto).unwrap())),
            )
        }
    }
}

#[test]
fn seen_files_are_the_same_as_reading_the_whole_log() {
    let scratch = Scratch::new("same");
    let mut compared = 0;
    let mut nonempty = 0;
    for seed in 0..seeds() {
        let mut dice = Dice(seed * 2_654_435_761 + 5);
        let forked = dice.odds(5);
        let steps = 10 + dice.below(150);

        let parent = forked.then(|| UndoLog::run(&mut dice, steps, None));
        let parent_dir = scratch.0.join(format!("p{seed}"));
        let upto = parent.as_ref().map(|parent| {
            UndoLog::write(&parent_dir, parent.first, &parent.lines, &mut dice);
            parent.first + parent.lines.len() as u64 - 1
        });
        let base = parent
            .as_ref()
            .zip(upto)
            .map(|(parent, upto)| (upto, parent.turns.clone()));
        let steps = 20 + dice.below(400);

        let log = UndoLog::run(&mut dice, steps, base);
        assert_eq!(log.ledger(), None, "种子 {seed} 造的过得了账本");
        // 日志停在每一次撤销、恢复刚落盘（偶尔再跟几条），和整份的。
        let mut ends: Vec<usize> = log
            .undos
            .iter()
            .map(|&seq| (seq - log.first) as usize + 1 + dice.below(3) as usize)
            .collect();
        ends.push(log.lines.len());
        for (k, end) in ends.into_iter().enumerate() {
            let end = end.min(log.lines.len());
            let dir = scratch.0.join(format!("s{seed}-{k}"));
            UndoLog::write(&dir, log.first, &log.lines[..end], &mut dice);
            let store = open(&dir, upto.map(|upto| (parent_dir.as_path(), upto)));
            let (was, now) = both(store.as_ref());
            assert_eq!(now, was, "种子 {seed}，到第 {end} 行");
            compared += 1;
            nonempty += usize::from(was.is_ok_and(|seen| !seen.is_empty()));
            drop(store);
            drop(fs::remove_dir_all(&dir));
        }
        drop(fs::remove_dir_all(&parent_dir));
    }
    eprintln!("比了 {compared} 次，看过文件的 {nonempty} 次");
    assert!(
        nonempty * 2 > compared,
        "比了 {compared} 次，看过文件的 {nonempty} 次"
    );
}

/// 只照目录读、不打开的（坏了的日志打不开）：以前的 `read_events` 再算，和新的 `scan_own`。
fn by_dir(dir: &Path) -> (Result<Seen, String>, Result<Seen, String>) {
    let was = read_events(dir)
        .map(|events| old::seen_in(&events))
        .map_err(|error| error.to_string());
    let mut scan = SeenScan::default();
    let now = scan_own(dir, &mut scan)
        .map(|()| scan.seen())
        .map_err(|error| error.to_string());
    (was, now)
}

/// 坏一行（不是合法的 JSON）、少一行（序号断了）、不是最后一段的末尾多半行：报的错和以前一字不差；最后一段末尾的半行两边都
/// 跳过。
#[test]
fn broken_logs_report_the_same_error() {
    let scratch = Scratch::new("broken");
    let mut errors = 0;
    for seed in 0..seeds() / 4 {
        let mut dice = Dice(seed * 40_503 + 11);
        let steps = 20 + dice.below(300);

        let log = UndoLog::run(&mut dice, steps, None);
        let mut lines = log.lines.clone();
        let k = 1 + dice.below(lines.len() as u64 - 1) as usize;
        let half = match dice.below(3) {
            0 => {
                lines[k] = format!(r#"{{"seq":{},"at":"#, log.first + k as u64);
                false
            }
            1 => {
                lines.remove(k);
                false
            }
            _ => true,
        };
        let dir = scratch.0.join(format!("b{seed}"));
        UndoLog::write(&dir, log.first, &lines, &mut dice);
        if half {
            // 照名字挑一段（最后一段也可能），末尾多半行。
            let mut segments: Vec<PathBuf> = fs::read_dir(&dir)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect();
            segments.sort();
            let segment = &segments[dice.below(segments.len() as u64) as usize];
            let mut text = fs::read(segment).unwrap();
            text.extend_from_slice(br#"{"seq":1,"at":"#);
            fs::write(segment, text).unwrap();
        }
        let (was, now) = by_dir(&dir);
        assert_eq!(now, was, "种子 {seed}");
        errors += usize::from(was.is_err());
        drop(fs::remove_dir_all(&dir));
    }
    assert!(errors > 0, "坏了的 {errors}");
}

/// 和以前不一样的一：不要的种类（这里是她的回复）的正文合 JSON、不合那一种的写法的，以前报坏了、重算不出来（照旧用原来的），
/// 现在不解它、照算（同判官只解要的种类，2026-11-03 项目主人拍板接受）。要的种类（工具结果）坏了照报。外壳坏了照报。
#[test]
fn a_skipped_kind_with_a_wrong_body_is_not_reported() {
    let scratch = Scratch::new("skipped");
    let reply = r#"{"seq":2,"at":"2026-11-03T00:00:02.000Z","kind":"message.assistant","turn":2,"by":{"kind":"kernel"},"body":{"blocks":5,"seen":1}}"#;
    let result = r#"{"seq":2,"at":"2026-11-03T00:00:02.000Z","kind":"tool.result","turn":2,"by":{"kind":"kernel"},"body":{"call_id":5}}"#;
    let mut dice = Dice(1);
    let log = UndoLog::run(&mut dice, 40, None);
    for (name, line, differs) in [("reply", reply, true), ("result", result, false)] {
        let mut lines = log.lines.clone();
        lines.insert(1, line.to_string());
        for (k, line) in lines.iter_mut().enumerate().skip(2) {
            *line = line.replacen(&format!("\"seq\":{}", k), &format!("\"seq\":{}", k + 1), 1);
        }
        let dir = scratch.0.join(name);
        UndoLog::write(&dir, 1, &lines, &mut dice);
        let (was, now) = by_dir(&dir);
        assert!(was.is_err(), "{name}：以前报坏了");
        assert_eq!(now.is_ok(), differs, "{name}：{now:?}");
    }
}

/// 和以前不一样的二：分叉出来的会话的底子读到分叉点就停，分叉点以后坏了的（父会话后来写坏的那一截）以前报、现在不报（同
/// 施工 V-37 下 `Base::each`）。分叉点以前坏了的照报。
#[test]
fn a_base_broken_after_the_fork_point_is_not_read() {
    let scratch = Scratch::new("base");
    let mut dice = Dice(7);
    let parent = UndoLog::run(&mut dice, 120, None);
    let upto = 40;
    let child = UndoLog::run(&mut dice, 60, Some((upto, Vec::new())));
    // 分叉点以后再跟两万条（约 3 MB，一窗里分给好几个线程解）：坏的那一条离分叉点远，不和它在一块里。
    let mut lines: Vec<String> = parent.lines[..40].to_vec();
    for seq in 41..=20_040_u64 {
        lines.push(format!(
            r#"{{"seq":{seq},"at":"2026-11-03T00:00:00.000Z","kind":"message.user","by":{{"kind":"person"}},"body":{{"blocks":[{{"type":"text","text":"后来说的第 {seq} 句，凑长一点凑长一点凑长一点"}}]}}}}"#
        ));
    }
    for (name, broken, differs) in [("after", 20_000_u64, true), ("before", 20, false)] {
        let mut lines = lines.clone();
        lines[usize::try_from(broken).unwrap() - 1] = format!(r#"{{"seq":{broken},"at":"#);
        let parent_dir = scratch.0.join(format!("{name}-p"));
        fs::create_dir_all(&parent_dir).unwrap();
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        fs::write(parent_dir.join(format!("{:012}.jsonl", 1)), text).unwrap();
        let dir = scratch.0.join(name);
        UndoLog::write(&dir, child.first, &child.lines, &mut dice);
        let store = open(&dir, Some((parent_dir.as_path(), upto)));
        let (was, now) = both(store.as_ref());
        assert!(was.is_err(), "{name}：以前报坏了");
        assert_eq!(now.is_ok(), differs, "{name}：{now:?}");
    }
}
