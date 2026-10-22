//! 判官看的群聊记录（施工 O-24）：和她看到的一行一个写法；旁听的、开过回合的都收，睡着的不收；别的线、主线的 `[you]` 行
//! 都收；撤回只认 `msg` 以前的；条数照 `count` 从新往旧取；`msg` 不是场所消息的没有。时刻都是 `2026-09-25T07:00Z`。

use super::*;
use crate::test_support::*;

const MEMBER: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#;
const BRIDGE: &str = r#"{"kind":"module","id":"onebot"}"#;
const OWN: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";
const BRANCH: &str = "0192f3a0-1111-7abc-8def-001122334455";

fn said(log: &mut Log, text: &str, venue: &str) -> u64 {
    log.detached(
        MEMBER,
        "message.user",
        &format!(r#"{{"blocks":[{}],"venue":{venue}}}"#, text_json(text)),
    )
}

fn delivered(log: &mut Log, line: &str, msg: &str, text: &str) {
    log.detached(
        BRIDGE,
        "venue.delivered",
        &format!(r#"{{"line":"{line}","turn":3,"to":[],"msg":"{msg}","text":"{text}"}}"#),
    );
}

fn chat() -> GroupChat {
    group_texts(480).group.expect("群会话")
}

/// 一段群聊，交回日志和要判的那一条。
fn chatted() -> (Log, Seq) {
    let mut log = Log::new();
    log.owned_by(OWN);
    said(
        &mut log,
        "早",
        r#"{"msg":"8801","name":"小林","ambient":true}"#,
    );
    said(
        &mut log,
        "梦话",
        r#"{"msg":"8802","name":"小林","ambient":true,"asleep":true}"#,
    );
    // 补拉回来的同睡着时收到的，判官不看（施工 O-40）。
    said(
        &mut log,
        "断线时说的",
        r#"{"msg":"8799","name":"小林","ambient":true,"backfill":true}"#,
    );
    delivered(&mut log, OWN, "8803", "在");
    delivered(&mut log, BRANCH, "8804", "我来");
    said(
        &mut log,
        "@Miyu 在吗",
        r#"{"msg":"8805","name":"小林","mentions_me":true}"#,
    );
    log.detached(
        BRIDGE,
        "venue.recalled",
        r#"{"msg":"8801","by":"qq:20017"}"#,
    );
    let msg = said(
        &mut log,
        "判我",
        r#"{"msg":"8806","name":"小林","ambient":true}"#,
    );
    log.detached(
        BRIDGE,
        "venue.recalled",
        r#"{"msg":"8805","by":"qq:20017"}"#,
    );
    (log, Seq::new(msg).expect("合写法"))
}

#[test]
fn records_read_like_her_lines_and_end_before_the_message() {
    let (log, msg) = chatted();
    let got = records(log.history(), msg, 20, &chat(), None).expect("是场所消息");
    assert_eq!(
        got.records,
        concat!(
            "[15:00] 小林 [msg=8801] (recalled): 早\n",
            "[15:00] [you] [msg=8803]: 在\n",
            "[15:00] [you] [msg=8804]: 我来\n",
            "[15:00] 小林 [msg=8805]: @Miyu 在吗\n  @mentions: [you]\n",
        )
    );
    assert_eq!(got.current, "[15:00] 小林 [msg=8806]: 判我");
}

#[test]
fn the_count_keeps_the_newest() {
    let (log, msg) = chatted();
    let got = records(log.history(), msg, 2, &chat(), None).expect("是场所消息");
    assert_eq!(
        got.records,
        "[15:00] [you] [msg=8804]: 我来\n[15:00] 小林 [msg=8805]: @Miyu 在吗\n  @mentions: [you]\n"
    );
}

#[test]
fn only_a_venue_message_can_be_judged() {
    let (mut log, _) = chatted();
    let plain = log.say("不是场所的");
    assert!(records(log.history(), Seq::new(plain).unwrap(), 20, &chat(), None).is_none());
    assert!(records(log.history(), Seq::new(1).unwrap(), 20, &chat(), None).is_none());
    // 前面一条都没有的：记录是空的。
    let mut fresh = Log::new();
    let first = said(
        &mut fresh,
        "第一句",
        r#"{"msg":"9000","name":"小林","ambient":true}"#,
    );
    let got =
        records(fresh.history(), Seq::new(first).unwrap(), 20, &chat(), None).expect("是场所消息");
    assert_eq!(got.records, "");
}

/// 老王引用 `reply_to` 的一条，编号 `msg`。
fn quoting(log: &mut Log, msg: &str, reply_to: &str) -> Seq {
    let seq = said(
        log,
        "接着说",
        &format!(r#"{{"msg":"{msg}","name":"老王","ambient":true,"reply_to":"{reply_to}"}}"#),
    );
    Seq::new(seq).expect("合写法")
}

#[test]
fn the_judged_line_quotes_the_message_it_replies_to() {
    // 施工 O-41：要判的那一条写出引用的那条的发的人和内容，照日志里那一行的写法；她发的写 `[you]`；睡着时收到的、
    // 补拉回来的也认（不进记录，但日志里有）。
    let (mut log, _) = chatted();
    let msg = quoting(&mut log, "8901", "8805");
    let got = records(log.history(), msg, 20, &chat(), None).expect("是场所消息");
    assert_eq!(
        got.current,
        "[15:00] 老王 [msg=8901]: 接着说\n  reply-to: msg=8805 小林: @Miyu 在吗"
    );
    for (quoted, shown) in [
        ("8803", "8803 [you]: 在"),
        ("8804", "8804 [you]: 我来"),
        ("8802", "8802 小林: 梦话"),
        ("8799", "8799 小林: 断线时说的"),
    ] {
        let msg = quoting(&mut log, "8902", quoted);
        let got = records(log.history(), msg, 20, &chat(), None).expect("是场所消息");
        assert!(
            got.current.ends_with(&format!("\n  reply-to: msg={shown}")),
            "{quoted}: {}",
            got.current
        );
    }
}

#[test]
fn a_quote_reads_like_its_own_line() {
    // 身份照引用的那条的 `show_ids`；名字、内容照不可信的转义；内容截到一行正文的上限；带的东西照记号写。
    let mut log = Log::new();
    let long = "长".repeat(2000);
    said(
        &mut log,
        &format!("  {long}\n"),
        r#"{"msg":"8001","name":"小林","show_ids":true,"media":[{"kind":"image","id":"i"}]}"#,
    );
    said(&mut log, "x\ny", r#"{"msg":"8002","name":"a\nb"}"#);
    let msg = quoting(&mut log, "8003", "8001");
    let got = records(log.history(), msg, 20, &chat(), None).expect("是场所消息");
    let cut = "长".repeat(4096 / 3);
    assert_eq!(
        got.current,
        format!(
            "[15:00] 老王 [msg=8003]: 接着说\n  reply-to: msg=8001 小林 (id=qq:20017): {cut} [image]"
        )
    );
    let msg = quoting(&mut log, "8004", "8002");
    let got = records(log.history(), msg, 20, &chat(), None).expect("是场所消息");
    let b = '\\';
    assert!(
        got.current
            .ends_with(&format!("\n  reply-to: msg=8002 a{b}nb: x{b}ny")),
        "{}",
        got.current
    );
}

#[test]
fn a_quote_not_before_the_judged_line_is_only_its_number() {
    // 日志里没有的、在要判的那一条以后才记下的，照旧只写编号；之前的记录里引用的那几行也照旧只写编号。
    let (mut log, _) = chatted();
    let missing = quoting(&mut log, "8901", "7777");
    let later = quoting(&mut log, "8902", "8903");
    said(&mut log, "后来的", r#"{"msg":"8903","name":"小林"}"#);
    let judged = quoting(&mut log, "8904", "8805");
    let got = records(log.history(), missing, 20, &chat(), None).expect("是场所消息");
    assert_eq!(
        got.current,
        "[15:00] 老王 [msg=8901]: 接着说\n  reply-to: msg=7777"
    );
    let got = records(log.history(), later, 20, &chat(), None).expect("是场所消息");
    assert!(
        got.current.ends_with("\n  reply-to: msg=8903"),
        "{}",
        got.current
    );
    let got = records(log.history(), judged, 20, &chat(), None).expect("是场所消息");
    assert!(
        got.records
            .contains("[15:00] 老王 [msg=8901]: 接着说\n  reply-to: msg=7777\n")
            && got
                .records
                .contains("[15:00] 老王 [msg=8902]: 接着说\n  reply-to: msg=8903\n"),
        "{}",
        got.records
    );
}

#[test]
fn a_repeated_number_quotes_the_newest() {
    let mut log = Log::new();
    said(&mut log, "旧的", r#"{"msg":"8001","name":"小林"}"#);
    said(&mut log, "新的", r#"{"msg":"8001","name":"小林"}"#);
    let msg = quoting(&mut log, "8002", "8001");
    let got = records(log.history(), msg, 20, &chat(), None).expect("是场所消息");
    assert!(got.current.ends_with(" 小林: 新的"), "{}", got.current);
}

/// 照步长对齐起点（施工 O-44）：起点停在步长的倍数上，条数在 `count` 到 `count + grow − 1` 之间；不够的从头。
#[test]
fn a_window_with_a_step_starts_on_a_multiple_of_it() {
    let at = |count, grow, n| Window { count, grow }.start(n);
    assert_eq!(at(20, None, 50), 30, "没有步长的是最近的 count 条");
    assert_eq!(at(20, None, 5), 0);
    assert_eq!(at(20, Some(10), 50), 30);
    assert_eq!(at(20, Some(10), 59), 30, "又来了 9 条，起点不动");
    assert_eq!(at(20, Some(10), 60), 40, "满一步才往前挪");
    assert_eq!(at(20, Some(10), 19), 0, "不够 count 条的从头");
    assert_eq!(at(20, Some(0), 50), 30, "步长 0 照没有");
}

#[test]
fn records_in_a_window_say_how_many() {
    let (log, msg) = chatted();
    let got = records_in(
        log.history(),
        msg,
        Window {
            count: 1,
            grow: Some(2),
        },
        &chat(),
        None,
    )
    .expect("是场所消息");
    assert_eq!(got.count, 2, "4 条里起点对齐到第 2 条：{}", got.records);
    assert_eq!(
        got.records,
        "[15:00] [you] [msg=8804]: 我来\n[15:00] 小林 [msg=8805]: @Miyu 在吗\n  @mentions: [you]\n"
    );
    let plain = records(log.history(), msg, 20, &chat(), None).expect("是场所消息");
    assert_eq!(plain.count, 4);
}
