//! 私聊里的引用（施工 QT-2，项目主人 2026-11-16 定）：引用了的在最后单独一块写 `reply-to: msg=编号`，带了 `quoted` 的下面缩进
//! 2 格写出那一条、里面的转发展开，写法同群里那一行；快照里没有转发那几句的只写编号；没引用的一字不变。

use super::{before, heard, now};
use crate::render::render;
use crate::test_support::*;

/// 引用 `700`、带着引用的内容：那一条是阿杰说的「周六\n见」，带一张图，里面套一个转发。
const QUOTING: &str = r#"{"msg":"720","reply_to":"700","quoted":{"name":"阿杰","text":"周六\n见","media":[{"kind":"image","id":"i-1","size":2048,"index":1}],"forwards":[{"nodes":[{"name":"小美","text":"路线在这"}]}]}}"#;

/// 写出来的引用那一块。
const REPLY: &str = concat!(
    "reply-to: msg=700\n",
    "  阿杰: 周六\\n见 [image #1: 2 KB]\n",
    "    forward:\n",
    "      小美: 路线在这",
);

#[test]
fn a_private_quote_goes_under_reply_to_as_its_own_block() {
    let mut log = Log::new();
    heard(&mut log, &text_json("对"), QUOTING);
    assert_eq!(now(&log), [format!("user: 对 | {REPLY}")]);
    assert_eq!(
        before(&log),
        [format!("user: 对 | {REPLY}")],
        "O-34 起造的同样写"
    );
}

/// 被引用的那一条就在之前的日志里：照样写出原文（只看这一条自己带的）。
#[test]
fn a_private_quote_already_in_the_log_is_written_too() {
    let mut log = Log::new();
    heard(&mut log, &text_json("周六见"), r#"{"msg":"700"}"#);
    heard(&mut log, &text_json("对"), QUOTING);
    let shown = now(&log);
    assert!(
        shown
            .last()
            .is_some_and(|last| last.ends_with(&format!("对 | {REPLY}"))),
        "{shown:?}"
    );
}

/// 带的东西、转发的先后：正文、记号那一行、引用、转发；没带 `quoted` 的只写编号；快照里没有转发那几句的也只写编号。
#[test]
fn a_private_reply_keeps_its_place_and_without_quoted_has_only_the_number() {
    let mut log = Log::new();
    heard(
        &mut log,
        &text_json("看看 [forward]"),
        r#"{"msg":"721","reply_to":"700","media":[{"kind":"file","id":"f-1","name":"a.pdf"}],"forwards":[{"nodes":[{"name":"阿杰","text":"嗯"}]}]}"#,
    );
    assert_eq!(
        now(&log),
        [
            "user: 看看 [forward] | [msg=721] [file: a.pdf] | reply-to: msg=700 | forward:\n  阿杰: 嗯"
        ]
    );
    let mut old = Log::new();
    heard(&mut old, &text_json("对"), QUOTING);
    assert_eq!(
        shape(&render(old.history(), &texts())),
        ["user: 对 | reply-to: msg=700"],
        "O-34 以前造的不写引用的内容"
    );
}

/// 引用的内容里带的东西照桥给的编号标 `#n`（施工 QT-2 守着）；桥没给编号的（例如引用的是她自己说的那一条）不标。
#[test]
fn a_private_quote_keeps_the_media_numbers_the_bridge_gave() {
    let mut log = Log::new();
    heard(
        &mut log,
        &text_json("这两张"),
        r#"{"msg":"723","reply_to":"700","quoted":{"name":"阿杰","text":"周六见[forward]","media":[{"kind":"image","id":"i-1","size":2048,"index":1}],"forwards":[{"nodes":[{"name":"小美","text":"图","media":[{"kind":"image","id":"i-2","size":1024,"index":2}]}]}]}}"#,
    );
    assert_eq!(
        now(&log),
        [concat!(
            "user: 这两张 | reply-to: msg=700\n",
            "  阿杰: 周六见[forward] [image #1: 2 KB]\n",
            "    forward:\n",
            "      小美: 图 [image #2: 1 KB]",
        )]
    );
    let mut own = Log::new();
    heard(
        &mut own,
        &text_json("你发的这张"),
        r#"{"msg":"724","reply_to":"701","quoted":{"name":"Miyu","text":"给你","media":[{"kind":"image","id":"i-3","size":2048}]}}"#,
    );
    assert_eq!(
        now(&own),
        ["user: 你发的这张 | reply-to: msg=701\n  Miyu: 给你 [image: 2 KB]"]
    );
}

/// 没引用的私聊消息一字不变（守老消息的前缀）。
#[test]
fn a_private_message_without_reply_to_is_unchanged() {
    let mut log = Log::new();
    heard(&mut log, &text_json("在吗"), r#"{"msg":"722"}"#);
    assert_eq!(now(&log), ["user: 在吗"]);
    assert_eq!(before(&log), ["user: 在吗"]);
}
