//! 只数行、不解（施工 V-41 上）：数到的每一行的序号、原文、位置和整份读的一样；照位置读一截、读一行，交的和整份读了再挑的
//! 一样，读到的照旧自检；数得出来的几样（段末半行、段名接不上、空着的最后一段名字不对）和从头读一样报坏了。

use std::fs;
use std::path::Path;

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use crate::log::{Flow, Mark, OpenError, read_between, read_events, read_line_near, scan_lines};
use crate::test_support::Scratch;

/// 第 `seq` 条的一行（不带换行），正文长短照序号变，有的很长（比一窗还长）。
fn line(seq: u64) -> String {
    let long = if seq.is_multiple_of(17) { 300_000 } else { 0 };
    let text = "句".repeat(usize::try_from(seq % 13).unwrap_or(0) + long);
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

/// 数到的：序号、原文、开头、下一行的开头。
fn scanned(dir: &Path, before: Option<u64>) -> Result<Vec<(u64, String, Mark, Mark)>, OpenError> {
    let mut got = Vec::new();
    scan_lines(dir, before, &mut |line| {
        assert_eq!(line.start.next, line.seq);
        got.push((
            line.seq.get(),
            String::from_utf8(line.text.to_vec()).unwrap(),
            line.start,
            line.end(),
        ));
        Flow::Go
    })?;
    Ok(got)
}

/// `read_between` 交的序号。
fn between(dir: &Path, from: &Mark, to: &Mark) -> Result<Vec<Event>, OpenError> {
    let mut got = Vec::new();
    read_between(dir, from, to, |events| {
        assert!(!events.is_empty(), "不交空的一块");
        got.extend(events);
        Flow::Go
    })?;
    Ok(got)
}

/// 三段（中间夹一段空的、末尾一行没写完），有的行比一窗还长。
fn three(dir: &Path) {
    let mut last = lines(71, 110);
    last.extend_from_slice(br#"{"seq":111,"at""#);
    write(
        dir,
        &[
            (1, lines(1, 40)),
            (41, lines(41, 70)),
            (70, Vec::new()),
            (71, last),
        ],
    );
}

#[test]
fn every_line_is_counted_like_reading_the_whole() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    let mut last = lines(71, 110);
    last.extend_from_slice(br#"{"seq":111,"at""#);
    write(&dir, &[(1, lines(1, 40)), (41, lines(41, 70)), (71, last)]);
    let whole = read_events(&dir).unwrap();
    let got = scanned(&dir, None).unwrap();
    assert_eq!(got.len(), whole.len());
    for ((seq, text, start, end), event) in got.iter().zip(&whole) {
        assert_eq!(*seq, event.seq.get());
        assert_eq!(*text, event.to_line());
        assert_eq!(
            start.segment,
            [1, 41, 71][usize::from(*seq > 40) + usize::from(*seq > 70)]
        );
        // 一行的开头读一行，就是它；一行一行接着读。
        assert_eq!(between(&dir, start, end).unwrap(), vec![event.clone()]);
        assert_eq!(
            read_line_near(&dir, start, event.seq).unwrap(),
            Some(event.clone())
        );
    }
    // 只交序号小于 `before` 的；`before` 在第一条以前的一行都不交，还是查有没有这个会话。
    for before in [1, 2, 40, 41, 42, 70, 71, 72, 110, 111, 500] {
        let got = scanned(&dir, Some(before)).unwrap();
        let want: Vec<u64> = (1..before.min(111)).collect();
        assert_eq!(got.iter().map(|one| one.0).collect::<Vec<_>>(), want);
    }
    assert!(matches!(
        scanned(&scratch.path().join("nope"), Some(1)),
        Err(OpenError::Missing(_))
    ));
}

#[test]
fn a_stretch_between_two_lines_reads_like_picking_from_the_whole() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    three(&dir);
    let whole = read_events(&dir).unwrap();
    let got = scanned(&dir, None).unwrap();
    for from in [0, 1, 20, 39, 40, 41, 69, 70, 71, 100, 109] {
        for to in [from, from + 1, 39, 40, 41, 69, 70, 75, 109] {
            if to < from || to >= got.len() {
                continue;
            }
            let read = between(&dir, &got[from].2, &got[to].3).unwrap();
            assert_eq!(read, whole[from..=to].to_vec(), "{from}..={to}");
        }
    }
    // 读到一行的末尾、一段的末尾为止的，叫停了不读下去。
    let mut blocks = 0;
    read_between(&dir, &got[0].2, &got[109].3, |_| {
        blocks += 1;
        Flow::Stop
    })
    .unwrap();
    assert_eq!(blocks, 1);
    // 照前面一行数过去读一行：同一段里的都找得到，别的段里的、更早的交回空的。
    for near in [0, 10, 39] {
        for target in [near, (near + 1).min(39), 39] {
            let event = read_line_near(&dir, &got[near].2, seq(target as u64 + 1)).unwrap();
            assert_eq!(event, Some(whole[target].clone()));
        }
        assert_eq!(read_line_near(&dir, &got[near].2, seq(41)).unwrap(), None);
    }
    assert_eq!(read_line_near(&dir, &got[10].2, seq(5)).unwrap(), None);
    assert_eq!(read_line_near(&dir, &got[80].2, seq(111)).unwrap(), None);
    // 不在一行开头的位置：读的时候日志变了，读一截报读写出错，读一行交回空的。
    let mut crooked = got[5].2;
    crooked.bytes += 1;
    assert!(matches!(
        between(&dir, &crooked, &got[8].3),
        Err(OpenError::Io(_))
    ));
    assert_eq!(read_line_near(&dir, &crooked, seq(7)).unwrap(), None);
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

/// 报的坏了：哪一段、第几行。
fn spot(error: OpenError) -> (String, usize) {
    match error {
        OpenError::Broken { segment, line, .. } => (
            segment.file_name().unwrap().to_string_lossy().into_owned(),
            line,
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn what_is_read_is_checked_and_what_is_counted_is_not() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    let second = replace_line(&lines(41, 70), 5, r#"{"seq":45,"at":"not a time""#);
    write(&dir, &[(1, lines(1, 40)), (41, second)]);
    // 整份读报第二段第 5 行；数的时候不解，照样数完。
    let whole = spot(read_events(&dir).unwrap_err());
    assert_eq!(whole, ("000000000041.jsonl".to_string(), 5));
    let got = scanned(&dir, None).unwrap();
    assert_eq!(got.len(), 70);
    // 读到那一行的，报的和整份读的一样；从段中间读起的行号照段开头数。
    assert_eq!(
        spot(between(&dir, &got[40].2, &got[69].3).unwrap_err()),
        whole
    );
    assert_eq!(
        spot(between(&dir, &got[42].2, &got[45].3).unwrap_err()),
        whole
    );
    assert_eq!(
        spot(between(&dir, &got[0].2, &got[69].3).unwrap_err()),
        whole
    );
    assert_eq!(
        spot(read_line_near(&dir, &got[40].2, seq(45)).unwrap_err()),
        whole
    );
    assert_eq!(spot(scan_line_event(&dir, 45).unwrap_err()), whole);
    // 没读到的不报。
    assert_eq!(between(&dir, &got[45].2, &got[69].3).unwrap().len(), 25);
    assert_eq!(between(&dir, &got[0].2, &got[43].3).unwrap().len(), 44);
    // 序号写错了的：读到的报。
    let wrong = replace_line(&lines(41, 70), 3, &line(44));
    write(&dir, &[(1, lines(1, 40)), (41, wrong)]);
    let whole = spot(read_events(&dir).unwrap_err());
    assert_eq!(whole, ("000000000041.jsonl".to_string(), 3));
    let got = scanned(&dir, None).unwrap();
    assert_eq!(
        spot(between(&dir, &got[40].2, &got[50].3).unwrap_err()),
        whole
    );
    assert_eq!(
        spot(read_line_near(&dir, &got[40].2, seq(43)).unwrap_err()),
        whole
    );
    assert_eq!(spot(scan_line_event(&dir, 43).unwrap_err()), whole);
}

/// 数到第 `seq` 条时照 [`crate::log::Line::event`] 解它。
fn scan_line_event(dir: &Path, seq: u64) -> Result<Event, OpenError> {
    let mut found = None;
    scan_lines(dir, None, &mut |line| {
        if line.seq.get() == seq {
            found = Some(line.event());
            return Flow::Stop;
        }
        Flow::Go
    })?;
    found.unwrap()
}

#[test]
fn what_can_be_counted_is_reported_like_reading_from_the_start() {
    let scratch = Scratch::new();
    // 不是最后一段的末尾有半行。
    let dir = scratch.path().join("half");
    let mut first = lines(1, 40);
    first.extend_from_slice(br#"{"seq":41"#);
    write(&dir, &[(1, first), (41, lines(41, 50))]);
    assert_eq!(
        spot(scanned(&dir, None).unwrap_err()),
        spot(read_events(&dir).unwrap_err())
    );
    // 有行的段名字接不上前面的行数。
    let dir = scratch.path().join("misnamed");
    write(&dir, &[(1, lines(1, 40)), (42, lines(42, 50))]);
    assert!(read_events(&dir).is_err());
    assert_eq!(
        spot(scanned(&dir, None).unwrap_err()),
        ("000000000042.jsonl".to_string(), 1)
    );
    // 空着的最后一段名字不是下一条的序号。
    let dir = scratch.path().join("empty");
    write(&dir, &[(1, lines(1, 40)), (45, Vec::new())]);
    assert_eq!(
        spot(scanned(&dir, None).unwrap_err()),
        spot(read_events(&dir).unwrap_err())
    );
    // 分叉出来的会话从分叉点数起。
    let dir = scratch.path().join("fork");
    write(&dir, &[(31, lines(31, 60))]);
    let got = scanned(&dir, None).unwrap();
    assert_eq!(got.first().map(|one| one.0), Some(31));
    assert_eq!(
        between(&dir, &got[0].2, &got[29].3).unwrap(),
        read_events(&dir).unwrap()
    );
    assert_eq!(scanned(&dir, Some(31)).unwrap().len(), 0);
    // 没有一段的：没有这个会话。
    let dir = scratch.path().join("bare");
    fs::create_dir_all(&dir).unwrap();
    assert!(matches!(scanned(&dir, None), Err(OpenError::Missing(_))));
}
