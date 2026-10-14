//! 带 `after` 读（施工 V-6 上）：从那里读起、停在完整的字后面、一次最多 128 KiB、过了结尾照结尾算。

use std::io::Cursor;

use super::*;

fn text(bytes: &[u8]) -> Box<dyn Text> {
    Box::new(Cursor::new(bytes.to_vec()))
}

#[test]
fn it_reads_on_from_after() {
    let all = "第一行\n第二行\n".as_bytes();
    let piece = read(text(all), 0, true);
    assert_eq!(piece.output, "第一行\n第二行\n");
    assert_eq!(piece.size, all.len() as u64);
    assert!(!piece.more);
    let first = "第一行\n".len() as u64;
    let rest = read(text(all), first, true);
    assert_eq!(rest.output, "第二行\n");
    assert_eq!(rest.size, all.len() as u64);
}

#[test]
fn half_a_char_waits_for_the_rest_unless_the_command_is_over() {
    let mut bytes = b"ab".to_vec();
    bytes.extend_from_slice(&"你".as_bytes()[..2]);
    let running = read(text(&bytes), 0, true);
    assert_eq!(
        (running.output.as_str(), running.size),
        ("ab", 2),
        "半个字留到下一次"
    );
    let over = read(text(&bytes), 0, false);
    assert_eq!(over.output, "ab\u{FFFD}", "结束了的照原样交");
    assert_eq!(over.size, 4);
}

#[test]
fn one_reply_is_at_most_the_cap_and_ends_on_a_whole_char() {
    // 一个字三个字节，上限不是三的倍数：截在字中间的那半个留到下一次。
    let bytes = "字".repeat(CAP / 3 + 10).into_bytes();
    let piece = read(text(&bytes), 0, false);
    assert!(piece.more);
    assert_eq!(piece.size % 3, 0);
    assert!(piece.size <= CAP as u64 && piece.size > CAP as u64 - 3);
    assert!(!piece.output.contains('\u{FFFD}'));
    let next = read(text(&bytes), piece.size, false);
    assert_eq!(next.size, bytes.len() as u64);
    assert!(!next.more);
    assert_eq!(piece.output.len() + next.output.len(), bytes.len());
}

#[test]
fn after_past_the_end_counts_as_the_end() {
    let piece = read(text(b"abc"), 10, true);
    assert_eq!(
        piece,
        Piece {
            output: String::new(),
            size: 3,
            more: false
        }
    );
}
