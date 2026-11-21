//! 回答终端的几样、去转义序列：照终端界面真发的字节喂。

use super::*;

fn answered(data: &[u8]) -> String {
    String::from_utf8(answer(data)).unwrap()
}

#[test]
fn nothing_asked_gets_nothing() {
    assert!(answer(b"hello \x1b[2J\x1b[1;1H world").is_empty());
}

#[test]
fn the_cursor_and_the_device_are_answered() {
    assert_eq!(answered(b"\x1b[6n"), "\x1b[1;1R");
    assert_eq!(answered(b"\x1b[c"), "\x1b[?62;22c");
    assert_eq!(answered(b"\x1b[0c"), "\x1b[?62;22c");
    assert_eq!(answered(b"\x1b[5n"), "\x1b[0n");
    assert_eq!(answered(b"\x1b[>q"), "\x1bP>|kitty(0.40.1)\x1b\\");
    assert_eq!(answered(b"\x1b[>0q"), "\x1bP>|kitty(0.40.1)\x1b\\");
    assert_eq!(answered(b"\x1b[?u"), "\x1b[?0u");
}

#[test]
fn colors_are_answered_like_xterm() {
    assert_eq!(
        answered(b"\x1b]10;?\x1b\\"),
        "\x1b]10;rgb:e5e5/e5e5/e5e5\x1b\\"
    );
    assert_eq!(
        answered(b"\x1b]11;?\x07"),
        "\x1b]11;rgb:1a1a/1b1b/2626\x1b\\"
    );
    assert_eq!(
        answered(b"\x1b]4;1;?\x07\x1b]4;15;?\x1b\\"),
        "\x1b]4;1;rgb:cdcd/3131/3131\x1b\\\x1b]4;15;rgb:ffff/ffff/ffff\x1b\\"
    );
    // 16 色以外的、不是在问的（设颜色）不答。
    assert!(answer(b"\x1b]4;16;?\x07").is_empty());
    assert!(answer(b"\x1b]4;1;rgb:00/00/00\x07").is_empty());
}

#[test]
fn sizes_follow_the_terminal() {
    assert_eq!(answered(b"\x1b[18t"), "\x1b[8;45;160t");
    assert_eq!(answered(b"\x1b[14t"), "\x1b[4;900;1600t");
    assert_eq!(answered(b"\x1b[16t"), "\x1b[6;20;10t");
}

#[test]
fn graphics_and_modes_are_answered_by_number() {
    assert_eq!(
        answered(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\"),
        "\x1b_Gi=31;OK\x1b\\"
    );
    // 不是 `a=q` 的（真的在画图）不答。
    assert!(answer(b"\x1b_Gi=31,a=T;AAAA\x1b\\").is_empty());
    assert_eq!(
        answered(b"\x1b[?2026$p\x1b[?2004$p"),
        "\x1b[?2026;2$y\x1b[?2004;2$y"
    );
    // 设模式（`h`）不是问。
    assert!(answer(b"\x1b[?2004h").is_empty());
}

#[test]
fn several_questions_in_one_block_are_all_answered() {
    let data = b"\x1b[?u\x1b[c\x1b]11;?\x1b\\\x1b[6n";
    let text = answered(data);
    for wanted in ["\x1b[?0u", "\x1b[?62;22c", "\x1b]11;", "\x1b[1;1R"] {
        assert!(text.contains(wanted), "{text:?}");
    }
}

#[test]
fn strip_leaves_what_is_drawn() {
    let drawn = b"\x1b[?2026h\x1b[1;1H\x1b[38;2;1;2;3m\xe2\x9d\xaf \x1b[0mperf\x1b]0;title\x07 ok\x1b(B\x1bP>|x\x1b\\\x1b_Ga=q\x1b\\\x1b7 end";
    assert_eq!(String::from_utf8(strip(drawn)).unwrap(), "❯ perf ok end");
}

#[test]
fn an_unfinished_sequence_is_cut_at_the_end() {
    assert_eq!(strip(b"abc\x1b"), b"abc");
    assert_eq!(strip(b"abc\x1b[38;2"), b"abc");
    assert_eq!(strip(b"abc\x1b]0;tit"), b"abc");
}

#[test]
fn shows_looks_through_the_colors() {
    let drawn = b"\x1b[1mmeasuring\x1b[0m reply";
    assert!(shows(drawn, "measuring reply"));
    assert!(!shows(drawn, "measuring replies"));
    assert!(shows(drawn, ""));
    // 词和词之间的空格是移光标跳过去的（终端界面真这样画）。
    let moved = b"\x1b[22;45Hperf-say:\x1b[22;68Hkeep\x1b[22;73Hgoing.";
    assert!(shows(moved, "perf-say: keep going."));
    assert!(!shows(moved, "perf-say: keep coming."));
}

#[test]
fn the_tail_is_the_last_characters() {
    assert_eq!(tail("一二三\x1b[1m四五".as_bytes(), 3), "三四五");
    assert_eq!(tail(b"ab", 10), "ab");
}
