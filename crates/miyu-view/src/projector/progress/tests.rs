//! 认进度行（施工 X-45 补）：切在两段里的拼得起来，只认前缀，几行取最后一条，去空白、空的不算，截长，半行不涨。

use super::{HELD, LONGEST, Line};

#[test]
fn a_line_cut_across_two_pieces_comes_together() {
    let mut line = Line::default();
    assert_eq!(line.feed("下载中\nmiyu-pro"), None);
    assert_eq!(line.feed("gress: 3/10\n"), Some("3/10".to_string()));
}

#[test]
fn only_whole_lines_with_the_exact_prefix_count() {
    let mut line = Line::default();
    let text = "x miyu-progress: no\nmiyu-progress:no\nMiyu-progress: no\n miyu-progress: no\n";
    assert_eq!(line.feed(text), None);
}

#[test]
fn the_last_line_of_a_piece_wins() {
    let mut line = Line::default();
    let text = "miyu-progress: 1/3\nmiyu-progress: 2/3\nsaved a.mp4\nmiyu-progress: 3/3\n";
    assert_eq!(line.feed(text), Some("3/3".to_string()));
}

#[test]
fn spaces_are_trimmed_and_empty_lines_do_not_count() {
    let mut line = Line::default();
    assert_eq!(
        line.feed("miyu-progress:   5/9 \r\n"),
        Some("5/9".to_string())
    );
    assert_eq!(line.feed("miyu-progress:    \r\n"), None);
}

#[test]
fn a_long_line_is_cut_to_its_first_characters() {
    let mut line = Line::default();
    let long = format!("miyu-progress: {}\n", "下".repeat(300));
    let value = line.feed(&long).expect("是进度行");
    assert_eq!(value.chars().count(), LONGEST);
}

#[test]
fn half_lines_do_not_grow() {
    let mut line = Line::default();
    assert_eq!(
        line.feed(&format!("miyu-progress: {}", "a".repeat(5000))),
        None
    );
    assert!(line.half.len() <= HELD, "{}", line.half.len());
    assert_eq!(line.feed("\n").map(|value| value.len()), Some(LONGEST));
    // 不是进度行的长行一个字节都不留。
    assert_eq!(line.feed(&"x".repeat(5000)), None);
    assert!(line.half.is_empty());
    assert_eq!(
        line.feed("\nmiyu-progress: next\n"),
        Some("next".to_string())
    );
}
