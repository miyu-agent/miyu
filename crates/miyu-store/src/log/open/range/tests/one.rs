//! 只读一条（施工 V-41 中，`read_one`）：落在各段的各处、空段上、定位对不上的、分叉出来的、还没写到的，交回的和 `read_range`
//! 从它读到它的一样，报的错一样；只有定位到了的不解它后面的行，后面坏了不报（`read_range` 照一窗解，可能报）。

use std::fs;
use std::path::Path;

use crate::log::{Flow, OpenError, read_one, read_range};
use crate::test_support::Scratch;

use super::{line, lines, replace_line, seq, write};

/// `read_range` 从 `n` 读到 `n` 交回的那一条。
fn ranged_one(dir: &Path, n: u64) -> Result<Option<u64>, String> {
    let mut got = None;
    read_range(dir, seq(n), Some(seq(n)), |events| {
        got = events.first().map(|event| event.seq.get());
        Flow::Go
    })
    .map_err(|error| format!("{error:?}"))?;
    Ok(got)
}

/// `read_one` 交回的那一条。
fn one(dir: &Path, n: u64) -> Result<Option<u64>, String> {
    read_one(dir, seq(n))
        .map(|event| event.map(|event| event.seq.get()))
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn every_seq_reads_like_a_range_of_one() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    let mut last = lines(71, 110);
    last.extend_from_slice(br#"{"seq":111,"at""#);
    write(&dir, &[(1, lines(1, 40)), (41, lines(41, 70)), (71, last)]);
    // 空段上的、行数不够的、那一行不是它的：退回从头读。
    let odd = scratch.path().join("o");
    write(
        &odd,
        &[
            (1, lines(1, 20)),
            (21, Vec::new()),
            (25, lines(21, 30)),
            (31, replace_line(&lines(31, 40), 4, &line(99))),
        ],
    );
    // 中间一个空段，名字落在前一段里：定位到它上面，退回从头读、读得到。
    let empty = scratch.path().join("e");
    write(
        &empty,
        &[(1, lines(1, 20)), (15, Vec::new()), (21, lines(21, 40))],
    );
    // 分叉出来的：第一段从第 9 条起。
    let forked = scratch.path().join("f");
    write(&forked, &[(9, lines(9, 30)), (31, Vec::new())]);
    let mut after = 0;
    for dir in [&dir, &odd, &empty, &forked] {
        for n in 1..=113 {
            let (got, ranged) = (one(dir, n), ranged_one(dir, n));
            if got == Ok(Some(n)) && ranged.is_err() {
                // 坏在它后面：`read_range` 解那一窗报出来，`read_one` 不解。
                after += 1;
                continue;
            }
            assert_eq!(got, ranged, "{} 第 {n} 条", dir.display());
        }
    }
    assert!(after > 0, "坏在后面的也走到");
    assert_eq!(one(&empty, 16), Ok(Some(16)), "退回从头读的读得到");
    assert!(matches!(
        read_one(&scratch.path().join("nobody"), seq(1)),
        Err(OpenError::Missing(_))
    ));
}

#[test]
fn lines_after_the_one_are_not_read() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("s");
    write(&dir, &[(1, replace_line(&lines(1, 30), 12, "not json"))]);
    assert_eq!(one(&dir, 11), Ok(Some(11)), "定位到了的只解那一行");
    assert_eq!(one(&dir, 30), Ok(Some(30)), "坏在前面的只数换行");
    assert!(one(&dir, 12).is_err(), "那一行自己坏了，退回从头读照报");
    fs::remove_dir_all(&dir).unwrap();
}
