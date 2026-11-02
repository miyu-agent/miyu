//! 一窗窗读和以前整段读进来再解的一样（施工 V-37 中）：同一批日志新旧各读一遍，交出去的每一条（`to_line()`）、一段读完的
//! 先后、报错（哪一段、第几行、为什么）、读到了哪里、截半行以后的文件逐个比。窗口调到几个字节，逼出窗口边上的几种：切在
//! 一行中间、切在换行上、切在 `\r\n` 中间、一行比窗口还长、坏行和序号断在窗口边上；从记下的位置读起的也比。以前那一份一字
//! 不改留在 `old.rs`。真数据形状的对照在 `real.rs`（`#[ignore]`，要 `MIYU_V37_ROOT`）。

mod cases;
mod old;
mod real;

use std::fs;
use std::path::Path;

use miyu_kernel::id::Seq;

use self::cases::{HALF, Segments, broken, fine, halves, line, lines, replace_line, shapes};
use super::{Piece, WINDOW, walk_in};
use crate::log::open::parse::Flow;
use crate::log::open::{HalfLine, Mark, OpenError};
use crate::test_support::Scratch;

/// 照 `segments` 在 `dir` 写好一个会话的日志（再放一个不是段的文件，不该看）。
fn write(dir: &Path, segments: &Segments) {
    fs::create_dir_all(dir).unwrap();
    for (first, bytes) in segments {
        fs::write(dir.join(format!("{first:012}.jsonl")), bytes).unwrap();
    }
    fs::write(dir.join("notes.txt"), "不是段").unwrap();
}

/// 目录里各个文件的名字和字节，照名字排。
fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut all: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            (name, fs::read(entry.path()).unwrap())
        })
        .collect();
    all.sort();
    all
}

/// 读的一种：以前的或者现在的。
type Reader<'a> = &'a dyn Fn(
    &Path,
    HalfLine,
    Option<&Mark>,
    &mut dyn FnMut(Piece) -> Flow,
) -> Result<Option<Mark>, OpenError>;

/// 以前整段读的。
fn before(
    dir: &Path,
    half: HalfLine,
    from: Option<&Mark>,
    sink: &mut dyn FnMut(Piece) -> Flow,
) -> Result<Option<Mark>, OpenError> {
    old::walk(dir, half, from, sink)
}

/// 什么时候叫停。
#[derive(Clone, Copy, Debug)]
enum Stop {
    /// 不叫停。
    Never,
    /// 第几段读完时叫停。
    AfterSegment(usize),
    /// 收到带这一条的那一块时叫停（块怎么分新旧不一样，只比到这一条）。
    AtSeq(u64),
}

/// 读一遍交出去的、交回的。
type Outcome = (Vec<Option<(u64, String)>>, String);

/// 读一遍：交出去的一条条（序号和 `to_line()`，一段读完记一个空的），和交回的写成字。
fn read(
    reader: Reader<'_>,
    dir: &Path,
    half: HalfLine,
    from: Option<&Mark>,
    stop: Stop,
) -> Outcome {
    let mut got = Vec::new();
    let mut ends = 0;
    let result = reader(dir, half, from, &mut |piece| match piece {
        Piece::Events(events) => {
            let seen =
                matches!(stop, Stop::AtSeq(at) if events.iter().any(|event| event.seq.get() == at));
            got.extend(
                events
                    .iter()
                    .map(|event| Some((event.seq.get(), event.to_line()))),
            );
            if seen { Flow::Stop } else { Flow::Go }
        }
        Piece::SegmentEnd => {
            got.push(None);
            ends += 1;
            if matches!(stop, Stop::AfterSegment(n) if n == ends) {
                Flow::Stop
            } else {
                Flow::Go
            }
        }
    });
    if let Stop::AtSeq(at) = stop {
        // 收到那一条以后的不比：那一块里它后面还有几条，看块怎么分。
        let seen = got
            .iter()
            .position(|one| one.as_ref().is_some_and(|(seq, _)| *seq == at));
        if let Some(k) = seen {
            got.truncate(k + 1);
        }
    }
    (got, told(result))
}

/// 交回的写成字：报错只写段的文件名（截半行的新旧在两个目录里截）。
fn told(result: Result<Option<Mark>, OpenError>) -> String {
    match result {
        Ok(end) => format!("读到 {end:?}"),
        Err(OpenError::Broken { segment, line, why }) => format!(
            "{} 第 {line} 行：{why}",
            segment.file_name().unwrap_or_default().to_string_lossy()
        ),
        Err(OpenError::Missing(_)) => "没有这个会话".to_string(),
        Err(OpenError::Io(error)) => format!("读写出错：{:?}", error.kind()),
    }
}

/// 新旧读到的一样。报错的：报的一样，读完了的段交的一样；报错的那一段里错以前交了几条看块怎么分（序号接不上的那一块整块不交，
/// 块怎么分以前就照核数不一样；交了的，调的一方见了错也都扔掉），只要一方是另一方的开头。各段都整个在一窗里的（`exact`），
/// 块和以前一模一样，这几条也要一样。
fn agree(old: &Outcome, new: &Outcome, exact: bool, what: &str) {
    assert_eq!(old.1, new.1, "{what}：交回的不一样");
    if exact || old.1.starts_with("读到") {
        assert!(old.0 == new.0, "{what}：交出去的不一样");
        return;
    }
    let done =
        |got: &[Option<(u64, String)>]| got.iter().rposition(Option::is_none).map_or(0, |k| k + 1);
    let (a, b) = (done(&old.0), done(&new.0));
    assert!(old.0[..a] == new.0[..b], "{what}：读完了的段交的不一样");
    let (x, y) = (&old.0[a..], &new.0[b..]);
    let shorter = x.len().min(y.len());
    assert!(
        x[..shorter] == y[..shorter],
        "{what}：报错的那一段错以前交的不一样"
    );
}

/// 各段都整个在一窗里。
fn fits(segments: &Segments, window: usize) -> bool {
    segments.iter().all(|(_, bytes)| bytes.len() <= window)
}

/// 窗口：从一个字节起的几种，加上照第一行定的几种（切在 `\r` 和 `\n` 中间、正好切在换行上、换行后一个）。
fn windows(segments: &Segments) -> Vec<usize> {
    let mut all = vec![1, 2, 3, 7, 63, 64, 100, 257, 4096, 65_536, WINDOW];
    if let Some(first) = segments
        .iter()
        .find_map(|(_, bytes)| memchr::memchr(b'\n', bytes))
    {
        let line = first + 1;
        all.extend([line.saturating_sub(2), line - 1, line, line + 1]);
    }
    all.retain(|window| *window > 0);
    all.sort_unstable();
    all.dedup();
    all
}

/// 每一段里几行的开头（头两行、末两行、隔 `every` 行一行），加几个对不上的位置（一行中间、末尾以后、没有的段）。
fn marks(segments: &Segments, every: usize) -> Vec<Mark> {
    let mut all = Vec::new();
    for (first, bytes) in segments {
        let starts: Vec<usize> = std::iter::once(0)
            .chain(memchr::memchr_iter(b'\n', bytes).map(|at| at + 1))
            .collect();
        for (k, start) in starts.iter().enumerate() {
            if k < 2 || k + 2 >= starts.len() || k % every == 0 {
                all.push(Mark {
                    segment: *first,
                    bytes: *start as u64,
                    next: Seq::new(first + k as u64).unwrap(),
                });
            }
        }
        let len = bytes.len() as u64;
        for bytes in [len / 2 + 1, len + 1, len + 7] {
            all.push(Mark {
                segment: *first,
                bytes,
                next: Seq::new(*first).unwrap(),
            });
        }
    }
    all.push(Mark {
        segment: 12_345,
        bytes: 0,
        next: Seq::FIRST,
    });
    all
}

/// 照以前的读法从头读得通：叫停在某一条的只比这几种（错在叫停的那一块里时报不报，看块怎么分，以前就照核数不一样）。
fn clean(segments: &Segments) -> bool {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    write(&dir, segments);
    before(&dir, HalfLine::Skip, None, &mut |_| Flow::Go).is_ok()
}

/// 新旧一样：只读地从头读、各种叫停，一个字节不写；截半行的在两份一样的拷贝上各截一遍，交回的、截出来的文件都一样；
/// 从记下的位置读起（窗口是 `mark_windows` 里的几种时，每隔 `every` 行一个位置）。一两个字节的窗口一个字节读一次，只从头
/// 读到底，不叫停；不 `thorough` 的（大的日志）也只比读到底和叫停在中间一条。
fn same(
    name: &str,
    segments: &Segments,
    windows: &[usize],
    mark_windows: &[usize],
    every: usize,
    thorough: bool,
) {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    write(&dir, segments);
    let written = files(&dir);
    let mut stops = vec![Stop::Never];
    if thorough {
        stops.extend([Stop::AfterSegment(1), Stop::AfterSegment(2)]);
    }
    if clean(segments) {
        let count = segments
            .iter()
            .map(|(_, bytes)| memchr::memchr_iter(b'\n', bytes).count() as u64)
            .sum::<u64>();
        let first = segments[0].0;
        if count > 0 && thorough {
            stops.extend([Stop::AtSeq(first), Stop::AtSeq(first + count - 1)]);
        }
        if count > 0 {
            stops.push(Stop::AtSeq(first + count / 2));
        }
    }
    let marks = marks(segments, every);
    for &window in windows {
        let now = move |dir: &Path,
                        half: HalfLine,
                        from: Option<&Mark>,
                        sink: &mut dyn FnMut(Piece) -> Flow| {
            walk_in(dir, half, from, window, sink)
        };
        let tiny = window < 7 || !thorough;
        let exact = fits(segments, window);
        for &stop in &stops {
            if tiny && !matches!(stop, Stop::Never) {
                continue;
            }
            let old = read(&before, &dir, HalfLine::Skip, None, stop);
            let new = read(&now, &dir, HalfLine::Skip, None, stop);
            agree(
                &old,
                &new,
                exact,
                &format!("{name}：只读，窗口 {window}，{stop:?}"),
            );
        }
        assert!(
            files(&dir) == written,
            "{name}：只读的一个字节不写，窗口 {window}"
        );
        for stop in [Stop::Never, Stop::AfterSegment(1)] {
            if tiny && !matches!(stop, Stop::Never) {
                continue;
            }
            let a = scratch.path().join(format!("old-{window}-{stop:?}"));
            let b = scratch.path().join(format!("new-{window}-{stop:?}"));
            write(&a, segments);
            write(&b, segments);
            let old = read(&before, &a, HalfLine::Cut, None, stop);
            let new = read(&now, &b, HalfLine::Cut, None, stop);
            agree(
                &old,
                &new,
                exact,
                &format!("{name}：截半行，窗口 {window}，{stop:?}"),
            );
            assert!(
                files(&a) == files(&b),
                "{name}：截出来的文件不一样，窗口 {window}，{stop:?}"
            );
        }
        if mark_windows.contains(&window) {
            for mark in &marks {
                let old = read(&before, &dir, HalfLine::Skip, Some(mark), Stop::Never);
                let new = read(&now, &dir, HalfLine::Skip, Some(mark), Stop::Never);
                agree(
                    &old,
                    &new,
                    exact,
                    &format!("{name}：从 {mark:?} 读起，窗口 {window}"),
                );
            }
        }
    }
}

/// 一批小日志，窗口从一个字节到默认的几种都比。
fn all_windows(cases: Vec<(&'static str, Segments)>) {
    for (name, segments) in cases {
        let windows = windows(&segments);
        same(name, &segments, &windows, &[3, 7, 64, WINDOW], 7, true);
    }
}

#[test]
fn fine_logs_read_the_same_in_small_windows() {
    all_windows(fine());
}

#[test]
fn half_lines_are_reported_and_cut_the_same_in_small_windows() {
    all_windows(halves());
}

#[test]
fn broken_logs_are_reported_the_same_in_small_windows() {
    all_windows(broken());
}

#[test]
fn long_lines_and_crlf_read_the_same_in_small_windows() {
    all_windows(shapes());
}

#[test]
fn a_missing_session_is_missing_either_way() {
    let scratch = Scratch::new();
    let nobody = scratch.path().join("nobody");
    let now =
        |dir: &Path, half: HalfLine, from: Option<&Mark>, sink: &mut dyn FnMut(Piece) -> Flow| {
            walk_in(dir, half, from, 7, sink)
        };
    assert_eq!(
        read(&before, &nobody, HalfLine::Skip, None, Stop::Never),
        read(&now, &nobody, HalfLine::Skip, None, Stop::Never)
    );
    fs::create_dir_all(&nobody).unwrap();
    fs::write(nobody.join("notes.txt"), "不是段").unwrap();
    let (_, said) = read(&now, &nobody, HalfLine::Cut, None, Stop::Never);
    assert_eq!(said, "没有这个会话");
}

/// 近一万条、几段、各种坏法；窗口有一窗里一个线程解的，也有一窗里几个线程一起解的（一窗到了 1 MiB 才分块）。
#[test]
fn ten_thousand_events_read_the_same() {
    let big = |tail: &[u8]| -> Segments {
        vec![
            (1, lines(1, 6_000)),
            (6_001, lines(6_001, 7_000)),
            (7_001, {
                let mut bytes = lines(7_001, 9_000);
                bytes.extend_from_slice(tail);
                bytes
            }),
        ]
    };
    let windows = [65_536, 1_100_000, WINDOW];
    let fine = big(b"");
    assert!(
        fine[0].1.len() > 1_100_000,
        "第一段要分得成两窗，头一窗几个线程一起解"
    );
    same("近一万条", &fine, &windows, &[WINDOW], 2_000, false);
    same(
        "近一万条、最后半行",
        &big(HALF.as_bytes()),
        &windows,
        &[],
        2_000,
        false,
    );
    let mut bad = big(b"");
    bad[0].1 = replace_line(&bad[0].1, 5_500, b"not json");
    same(
        "近一万条、第一段后一窗里一行坏了",
        &bad,
        &windows,
        &[],
        2_000,
        false,
    );
    let mut gap = big(b"");
    gap[0].1 = replace_line(&gap[0].1, 5_555, line(5_557, 5).as_bytes());
    same(
        "近一万条、序号断在第一段后一窗",
        &gap,
        &windows,
        &[],
        2_000,
        false,
    );
    let mut misnamed = big(HALF.as_bytes());
    misnamed[1].0 = 6_002;
    misnamed[1].1 = replace_line(&misnamed[1].1, 700, b"\xff");
    same(
        "近一万条、段名不对、后面又坏了",
        &misnamed,
        &windows,
        &[],
        2_000,
        false,
    );
}

/// 名字像段的目录：新旧都报读写出错，报的是系统说的那一个（Linux、macOS 上读的时候报，Windows 上打开时就报）。目录是空的，
/// 有的文件系统上长度是 0，也照样报，不当空段。
#[test]
fn a_directory_named_like_a_segment_fails_the_same() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    write(&dir, &vec![(1, lines(1, 10))]);
    fs::create_dir_all(dir.join("000000000011.jsonl")).unwrap();
    let now =
        |dir: &Path, half: HalfLine, from: Option<&Mark>, sink: &mut dyn FnMut(Piece) -> Flow| {
            walk_in(dir, half, from, 64, sink)
        };
    for half in [HalfLine::Skip, HalfLine::Cut] {
        let old = read(&before, &dir, half, None, Stop::Never);
        let new = read(&now, &dir, half, None, Stop::Never);
        assert_eq!(old, new);
        assert!(new.1.starts_with("读写出错"), "{}", new.1);
    }
}
