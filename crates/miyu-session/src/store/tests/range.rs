//! 读回照序号定位（施工 V-37 下）：撤掉压缩时读回第 `from` 条起的（`Store::events_from`）、分叉出来的会话的底子从第 `from`
//! 条读到分叉点（`Base::events_from`），回合索引接上时一块块读底子（`Base::each`），交回的、报的错都和原来一段段读、整个
//! 解完再挑的（照抄留在这里的 [`old_from`]、[`old_base`]）一样。`from` 以前的不解、不自检，读到分叉点就停（2026-11-03
//! 项目主人定）。

use std::fs;
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;
use miyu_store::log::{OpenError, SEGMENT_LIMIT, SessionLog, read_segments};

use super::super::{Base, Store};

/// 数据根里的临时目录（会话日志只在数据根里开，施工 TMP-1）：用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let path =
            std::env::temp_dir().join(format!("miyu-session-range-{name}-{}", std::process::id()));
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

/// 第 `seq` 条的一行（带换行），正文长短照序号变。
fn line(seq: u64) -> String {
    let text = "字".repeat(usize::try_from(seq % 11).unwrap_or(0));
    format!(
        "{{\"seq\":{seq},\"at\":\"2026-11-03T07:00:00.000Z\",\"kind\":\"message.user\",\"by\":{{\"kind\":\"person\"}},\"body\":{{\"blocks\":[{{\"type\":\"text\",\"text\":\"第 {seq} {text}\"}}]}}}}\n"
    )
}

fn lines(from: u64, to: u64) -> String {
    (from..=to).map(line).collect()
}

fn write(dir: &Path, segments: &[(u64, String)]) {
    fs::create_dir_all(dir).unwrap();
    for (first, text) in segments {
        fs::write(dir.join(format!("{first:012}.jsonl")), text).unwrap();
    }
}

fn seq(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

/// 原来的 `SessionLog` 的 `Store::events_from`（施工 6-9 起，V-37 下以前），照抄。
fn old_from(dir: &Path, from: Seq) -> Result<Vec<Event>, String> {
    let mut kept = Vec::new();
    read_segments(dir, |segment| {
        kept.extend(segment.into_iter().filter(|event| event.seq >= from));
        true
    })
    .map_err(|error| error.to_string())?;
    Ok(kept)
}

/// 原来的 `Base::events_from`（施工 V-20 起，V-37 下以前），照抄。
fn old_base(dir: &Path, upto: Seq, from: Seq) -> Result<Vec<Event>, OpenError> {
    let mut events = Vec::new();
    if from > upto {
        return Ok(events);
    }
    let mut reached = false;
    read_segments(dir, |mut segment| {
        reached = segment.last().is_some_and(|event| event.seq >= upto);
        segment.retain(|event| event.seq >= from && event.seq <= upto);
        if events.is_empty() {
            events = segment;
        } else {
            events.extend(segment);
        }
        !reached
    })?;
    if !reached {
        return Err(OpenError::Missing(dir.to_path_buf()));
    }
    Ok(events)
}

/// 两份交回的一样：事件逐个相等，报的错写出来一样。
fn alike<E: std::fmt::Display>(
    new: &Result<Vec<Event>, E>,
    old: &Result<Vec<Event>, E>,
    what: &str,
) {
    match (new, old) {
        (Ok(new), Ok(old)) => assert_eq!(new, old, "{what}"),
        (Err(new), Err(old)) => assert_eq!(new.to_string(), old.to_string(), "{what}"),
        (new, old) => panic!(
            "{what}：新的 {:?}，原来 {:?}",
            new.as_ref().map(Vec::len).map_err(ToString::to_string),
            old.as_ref().map(Vec::len).map_err(ToString::to_string)
        ),
    }
}

/// 三段夹一段空的，最后一段末尾有半行（打开时截掉）：读回第 `from` 条起的，每一处都和原来一样。
#[test]
fn reading_back_from_every_seq_matches_the_whole_walk() {
    let scratch = Scratch::new("from");
    let dir = scratch.0.join("s");
    let mut last = lines(61, 90);
    last.push_str(r#"{"seq":91,"at""#);
    write(
        &dir,
        &[
            (1, lines(1, 30)),
            (31, lines(31, 60)),
            (45, String::new()),
            (61, last),
        ],
    );
    let (log, _) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    for from in 1..=93 {
        let new = Store::events_from(&log, seq(from));
        alike(&new, &old_from(&dir, seq(from)), &format!("从 {from} 起"));
    }
    // 分叉出来的会话（第一段从第 9 条起）：从分叉点以前读起的照样一样。
    let forked = scratch.0.join("f");
    write(&forked, &[(9, lines(9, 30)), (31, lines(31, 50))]);
    let (log, _) = SessionLog::open(&forked, SEGMENT_LIMIT).unwrap();
    for from in 1..=52 {
        let new = Store::events_from(&log, seq(from));
        alike(
            &new,
            &old_from(&forked, seq(from)),
            &format!("分叉的从 {from} 起"),
        );
    }
}

/// 坏的一行在读回的那一截里：报的错和原来一字不差；在 `from` 以前的不解、不报（原来要报，项目主人定的差别）。
#[test]
fn a_broken_line_before_from_is_skipped_and_one_inside_is_told_alike() {
    let scratch = Scratch::new("broken");
    let dir = scratch.0.join("s");
    write(&dir, &[(1, lines(1, 30)), (31, lines(31, 60))]);
    let (log, _) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    // 开着以后第一段被外人改坏了一行（开的时候整份查过，坏了的开不了）。
    let first = lines(1, 30).replacen(&line(12), "{\"seq\":12,\n", 1);
    write(&dir, &[(1, first)]);
    for from in [1, 5, 11, 12] {
        let new = Store::events_from(&log, seq(from));
        alike(&new, &old_from(&dir, seq(from)), &format!("从 {from} 起"));
        assert_eq!(
            new.expect_err("坏的那一行在里面"),
            "000000000001.jsonl line 12 is broken"
        );
    }
    for from in [13, 20, 31, 45, 60] {
        assert!(old_from(&dir, seq(from)).is_err(), "原来整份解，都报");
        let got = Store::events_from(&log, seq(from)).unwrap();
        let seqs: Vec<u64> = got.iter().map(|event| event.seq.get()).collect();
        assert_eq!(seqs, (from..=60).collect::<Vec<_>>(), "从 {from} 起");
    }
}

/// 底子：从每一条读到每一个分叉点，和原来一样；一块块交的（回合索引接上时）连起来就是从第一条读的那一份；父会话没了、
/// 没读到分叉点的报没有。
#[test]
fn a_base_reads_from_every_seq_to_every_fork_point_alike() {
    let scratch = Scratch::new("base");
    let parent = scratch.0.join("p");
    write(
        &parent,
        &[(1, lines(1, 30)), (31, lines(31, 60)), (61, lines(61, 70))],
    );
    for upto in [1, 2, 29, 30, 31, 45, 60, 61, 70] {
        let base = Base::new(parent.clone(), seq(upto));
        for from in 1..=72 {
            let new = base.events_from(seq(from));
            let old = old_base(&parent, seq(upto), seq(from));
            alike(&new, &old, &format!("从 {from} 到 {upto}"));
        }
        let mut each = Vec::new();
        base.each(|events| {
            assert!(!events.is_empty(), "不交空的一块");
            each.extend(events);
        })
        .unwrap();
        assert_eq!(
            each,
            old_base(&parent, seq(upto), Seq::FIRST).unwrap(),
            "一块块交的，到 {upto}"
        );
    }
    // 父会话还没写到分叉点（被截短了）、没了：报没有。
    let base = Base::new(parent.clone(), seq(80));
    for from in [1, 2, 40, 70, 71] {
        assert!(matches!(
            base.events_from(seq(from)),
            Err(OpenError::Missing(_))
        ));
        assert!(matches!(
            old_base(&parent, seq(80), seq(from)),
            Err(OpenError::Missing(_))
        ));
    }
    assert!(matches!(base.each(|_| {}), Err(OpenError::Missing(_))));
    let gone = Base::new(scratch.0.join("gone"), seq(10));
    for from in [1, 5] {
        alike(
            &gone.events_from(seq(from)),
            &old_base(&scratch.0.join("gone"), seq(10), seq(from)),
            "父会话没了",
        );
    }
    assert!(matches!(gone.each(|_| {}), Err(OpenError::Missing(_))));
}

/// 底子里坏了的一行：在要的那一截里的报的错一样；在 `from` 以前的（撤销读回，`from` 比第一条大）不报；分叉点以后的不读、
/// 不报（原来读到分叉点所在那一段的末尾，同一段里的要报）。
#[test]
fn a_broken_line_in_a_base_outside_the_range_is_not_read() {
    let scratch = Scratch::new("base-broken");
    let parent = scratch.0.join("p");
    let first = lines(1, 30).replacen(&line(20), "{\"seq\":20,\n", 1);
    write(&parent, &[(1, first), (31, lines(31, 60))]);
    let inside = Base::new(parent.clone(), seq(25));
    for from in [1, 10, 19, 20] {
        alike(
            &inside.events_from(seq(from)),
            &old_base(&parent, seq(25), seq(from)),
            &format!("从 {from} 起"),
        );
    }
    let before = Base::new(parent.clone(), seq(45));
    for from in [21, 25, 31] {
        assert!(old_base(&parent, seq(45), seq(from)).is_err());
        let got = before.events_from(seq(from)).unwrap();
        let seqs: Vec<u64> = got.iter().map(|event| event.seq.get()).collect();
        assert_eq!(seqs, (from..=45).collect::<Vec<_>>());
    }
    let after = Base::new(parent.clone(), seq(10));
    assert!(
        old_base(&parent, seq(10), Seq::FIRST).is_err(),
        "原来读完第一段"
    );
    let mut each = Vec::new();
    after.each(|events| each.extend(events)).unwrap();
    assert_eq!(each.len(), 10);
    assert!(after.events_from(Seq::FIRST).is_err(), "从第一条读的照原来");
    assert_eq!(after.events_from(seq(2)).unwrap().len(), 9);
}
