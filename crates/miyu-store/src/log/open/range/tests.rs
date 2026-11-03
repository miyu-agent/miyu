//! 照序号读一截（施工 V-37 中，`read_range`）：定位落在各段的各处，交出去的和整份读了再挑的一样；读过 `upto` 当场停，后面
//! 的段不打开；`from` 落在空段、定位以后那一行不是 `from` 的退回从头读；跳过的那一截不自检。

use std::fs;
use std::path::Path;

use miyu_kernel::id::Seq;

use crate::log::{Flow, OpenError, read_events, read_range};
use crate::test_support::Scratch;

/// 第 `seq` 条的一行（不带换行），正文长短照序号变。
fn line(seq: u64) -> String {
    let text = "句".repeat(usize::try_from(seq % 13).unwrap_or(0));
    format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{{"kind":"person"}},"body":{{"blocks":[{{"type":"text","text":"第 {seq} {text}"}}]}}}}"#
    )
}

/// 第 `from` 到第 `to` 条，一行一条。
fn lines(from: u64, to: u64) -> Vec<u8> {
    (from..=to)
        .flat_map(|seq| format!("{}\n", line(seq)).into_bytes())
        .collect()
}

/// 一段里第 `k` 行（从 1 数）换成 `with`。
fn replace_line(bytes: &[u8], k: usize, with: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for (n, one) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        if n + 1 == k {
            out.extend_from_slice(with.as_bytes());
            out.push(b'\n');
        } else {
            out.extend_from_slice(one);
        }
    }
    out
}

/// 照 `segments` 写好一个会话的日志。
fn write(dir: &Path, segments: &[(u64, Vec<u8>)]) {
    fs::create_dir_all(dir).unwrap();
    for (first, bytes) in segments {
        fs::write(dir.join(format!("{first:012}.jsonl")), bytes).unwrap();
    }
}

fn seq(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

/// `read_range` 交出去的序号，和交了几块。
fn ranged(dir: &Path, from: u64, upto: Option<u64>) -> Result<Vec<u64>, OpenError> {
    let mut got = Vec::new();
    read_range(dir, seq(from), upto.map(seq), |events| {
        assert!(!events.is_empty(), "不交空的一块");
        got.extend(events.iter().map(|event| event.seq.get()));
        Flow::Go
    })?;
    Ok(got)
}

/// 整份读了再挑：`from..=upto` 的序号。
fn picked(dir: &Path, from: u64, upto: Option<u64>) -> Vec<u64> {
    read_events(dir)
        .unwrap()
        .iter()
        .map(|event| event.seq.get())
        .filter(|seq| *seq >= from && upto.is_none_or(|upto| *seq <= upto))
        .collect()
}

#[test]
fn every_place_in_every_segment_reads_like_picking_from_the_whole() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    let mut last = lines(71, 110);
    last.extend_from_slice(br#"{"seq":111,"at""#);
    write(&dir, &[(1, lines(1, 40)), (41, lines(41, 70)), (71, last)]);
    for from in 1..=113 {
        for upto in [
            None,
            Some(from),
            Some(from + 3),
            Some(40),
            Some(41),
            Some(70),
            Some(200),
        ] {
            assert_eq!(
                ranged(&dir, from, upto).unwrap(),
                picked(&dir, from, upto),
                "从 {from} 到 {upto:?}"
            );
        }
    }
    assert_eq!(
        ranged(&dir, 30, Some(29)).unwrap(),
        Vec::<u64>::new(),
        "到的在起的前面"
    );
}

#[test]
fn reading_stops_past_upto_and_later_segments_are_not_opened() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    write(&dir, &[(1, lines(1, 40))]);
    // 第二段打不开（是个目录）：读到它就出错。
    fs::create_dir_all(dir.join("000000000041.jsonl")).unwrap();
    assert!(
        matches!(ranged(&dir, 1, None), Err(OpenError::Io(_))),
        "读到第二段就出错"
    );
    assert_eq!(
        ranged(&dir, 1, Some(20)).unwrap(),
        (1..=20).collect::<Vec<_>>()
    );
    assert_eq!(
        ranged(&dir, 35, Some(40)).unwrap(),
        (35..=40).collect::<Vec<_>>()
    );
    // `each` 叫停了也不读下去。
    let mut blocks = 0;
    read_range(&dir, seq(3), None, |_| {
        blocks += 1;
        Flow::Stop
    })
    .unwrap();
    assert_eq!(blocks, 1);
}

#[test]
fn from_on_an_empty_segment_falls_back_to_reading_from_the_start() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    // 第一段第 5 行坏了；中间一段空着，名字 20（空的段名字不查）。
    write(
        &dir,
        &[
            (1, replace_line(&lines(1, 30), 5, "not json")),
            (20, Vec::new()),
            (31, lines(31, 50)),
        ],
    );
    // 落在空段上：退回从头读，读到第一段的坏行就报。
    for from in [20, 22, 30] {
        match ranged(&dir, from, None) {
            Err(OpenError::Broken { segment, line, .. }) => {
                assert_eq!(segment, dir.join("000000000001.jsonl"));
                assert_eq!(line, 5);
            }
            other => panic!("从 {from} 读起应该退回从头读、报坏了：{other:?}"),
        }
    }
    // 对得上的只数换行跳过前面，跳过的那一截不自检：坏行在前面的不报。
    assert_eq!(
        ranged(&dir, 6, Some(12)).unwrap(),
        (6..=12).collect::<Vec<_>>()
    );
    assert_eq!(
        ranged(&dir, 31, None).unwrap(),
        (31..=50).collect::<Vec<_>>()
    );
}

#[test]
fn a_line_that_is_not_from_falls_back_to_reading_from_the_start() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    // 第二段名字不对（叫 35，第一条是 31）：照名字数到的那一行不是要的那一条。
    write(&dir, &[(1, lines(1, 30)), (35, lines(31, 60))]);
    for from in [32, 36, 40, 60] {
        match ranged(&dir, from, None) {
            Err(OpenError::Broken { segment, line, why }) => {
                assert_eq!(segment, dir.join("000000000035.jsonl"));
                assert_eq!(line, 1);
                assert!(why.contains("named 35"), "{why}");
            }
            other => panic!("从 {from} 读起应该退回从头读、报段名不对：{other:?}"),
        }
    }
    // 要的那一行读不成：也退回从头读，照从头读的报。
    let dir = scratch.path().join("t");
    write(&dir, &[(1, replace_line(&lines(1, 30), 12, "not json"))]);
    match ranged(&dir, 12, None) {
        Err(OpenError::Broken { line, .. }) => assert_eq!(line, 12),
        other => panic!("应该报第 12 行坏了：{other:?}"),
    }
}

#[test]
fn a_broken_line_after_from_is_numbered_from_the_segment_start() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    write(
        &dir,
        &[
            (1, lines(1, 10)),
            (11, replace_line(&lines(11, 60), 40, "not json")),
        ],
    );
    let whole = read_events(&dir).unwrap_err();
    let ranged = ranged(&dir, 30, None).unwrap_err();
    match (&whole, &ranged) {
        (
            OpenError::Broken {
                segment: a,
                line: m,
                ..
            },
            OpenError::Broken {
                segment: b,
                line: n,
                ..
            },
        ) => {
            assert_eq!(a, b);
            assert_eq!((m, n), (&40, &40), "同从头读的行号");
        }
        _ => panic!("应该都报坏了：{whole:?} {ranged:?}"),
    }
}

#[test]
fn beyond_the_end_and_before_the_start() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    // 分叉出来的：第一段从第 9 条起。
    write(&dir, &[(9, lines(9, 30)), (31, Vec::new())]);
    assert_eq!(
        ranged(&dir, 1, Some(12)).unwrap(),
        (9..=12).collect::<Vec<_>>()
    );
    assert_eq!(
        ranged(&dir, 31, None).unwrap(),
        Vec::<u64>::new(),
        "下一条还没有"
    );
    assert_eq!(ranged(&dir, 99, None).unwrap(), Vec::<u64>::new());
    // 比最后一条还大的只数换行，不退回从头读：前面坏了也不报。
    let broken = scratch.path().join("t");
    write(&broken, &[(1, replace_line(&lines(1, 30), 3, "not json"))]);
    assert_eq!(ranged(&broken, 31, None).unwrap(), Vec::<u64>::new());
    assert_eq!(ranged(&broken, 40, None).unwrap(), Vec::<u64>::new());
    assert!(matches!(
        ranged(&scratch.path().join("nobody"), 1, None),
        Err(OpenError::Missing(_))
    ));
}

mod one;
mod only;
