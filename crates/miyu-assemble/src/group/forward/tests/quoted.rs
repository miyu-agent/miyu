//! 引用的那一条的内容（施工 QT-1）：这一条以前的日志里有被引用的那一条的不写，没有的在 `reply-to` 下面缩进写、里面的转发展开；
//! 没带 `quoted` 的、快照里没有转发那几句的一字不变；只看这一条以前的，后来才记下的不改它，同一份日志渲染两次一样；判官看的
//! 记录照同一个规矩。时刻都是 `2026-09-25T07:00Z`。

use miyu_kernel::id::Seq;

use super::{MEMBER, heard, in_group};
use crate::group::records;
use crate::render::render;
use crate::test_support::*;

/// 引用 `reply_to`、带着引用的内容的场所的格：那一条是阿杰说的「周六\n见」，带一张图，里面套一个转发。
fn quoting(msg: &str, reply_to: &str) -> String {
    format!(
        r#"{{"msg":"{msg}","name":"小林","reply_to":"{reply_to}","mentions_me":true,"quoted":{{"name":"阿杰","text":"周六\n见","media":[{{"kind":"image","id":"i-1","size":2048,"index":1}}],"forwards":[{{"nodes":[{{"name":"小美","text":"路线在这"}}]}}]}}}}"#
    )
}

/// 引用的那一条不在之前的日志里：写在 `reply-to` 下面，再缩进一层；里面的转发同转发的写法展开。
const UNSEEN: &str = concat!(
    "[15:00] 小林 [msg=8811]: 对\n",
    "  reply-to: msg=7000\n",
    "    阿杰: 周六\\n见 [image #1: 2 KB]\n",
    "      forward:\n",
    "        小美: 路线在这\n",
    "  @mentions: [you]",
);

#[test]
fn a_quote_missing_from_the_log_goes_under_reply_to_with_its_forwards() {
    let mut log = Log::new();
    heard(&mut log, MEMBER, "对", &quoting("8811", "7000"));
    assert_eq!(in_group(&log), [format!("user: {UNSEEN}")]);
}

#[test]
fn a_quote_already_in_the_log_is_not_written_again() {
    let mut log = Log::new();
    heard(
        &mut log,
        MEMBER,
        "周六见",
        r#"{"msg":"7000","name":"阿杰","ambient":true}"#,
    );
    heard(&mut log, MEMBER, "对", &quoting("8811", "7000"));
    assert_eq!(
        in_group(&log),
        [concat!(
            "user: [15:00] 小林 [msg=8811]: 对\n",
            "  reply-to: msg=7000\n",
            "  @mentions: [you]",
        )]
    );
}

/// 被引用的那一条只以补拉回来的样子在日志里（主会话 2026-11-14 定）：补拉回来的不进群聊近况，她那一轮看不到，照没有算，写出
/// 引用的内容。判官的 `current` 同一个规矩：编号后面照旧写日志里那一条（施工 O-41），下面也写引用的内容。
#[test]
fn a_quote_only_backfilled_is_written_anyway() {
    let mut log = Log::new();
    heard(
        &mut log,
        MEMBER,
        "周六见",
        r#"{"msg":"7000","name":"阿杰","ambient":true,"backfill":true}"#,
    );
    let msg = log.push(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{}}}"#,
            text_json("对"),
            quoting("8811", "7000")
        ),
    );
    assert_eq!(in_group(&log), [format!("user: {UNSEEN}")]);
    let texts = with_forward(group_texts(480));
    let chat = texts.group.clone().expect("群会话");
    let at = Seq::new(msg).expect("合写法");
    let got = records(log.history(), at, 20, &chat, texts.forward.as_ref()).expect("有");
    assert_eq!(
        got.current,
        UNSEEN.replace("msg=7000\n", "msg=7000 阿杰: 周六见\n")
    );
}

/// 没带 `quoted` 的、快照里没有转发那几句的：和以前一个字节不差。
#[test]
fn without_a_quote_or_the_forward_texts_nothing_changes() {
    let mut log = Log::new();
    heard(
        &mut log,
        MEMBER,
        "对",
        r#"{"msg":"8811","name":"小林","reply_to":"7000","mentions_me":true}"#,
    );
    let plain = concat!(
        "user: [15:00] 小林 [msg=8811]: 对\n",
        "  reply-to: msg=7000\n",
        "  @mentions: [you]",
    );
    assert_eq!(in_group(&log), [plain]);
    let mut old = Log::new();
    heard(&mut old, MEMBER, "对", &quoting("8811", "7000"));
    assert_eq!(
        shape(&render(old.history(), &group_texts(480))),
        [plain],
        "O-34 以前造的会话不写"
    );
}

/// 只看这一条以前的：被引用的那一条后来才记下（补拉回来的），这一条照旧写着引用的内容；同一份日志渲染两次字节一样。
#[test]
fn a_quote_recorded_later_does_not_change_the_line() {
    let mut log = Log::new();
    heard(&mut log, MEMBER, "对", &quoting("8811", "7000"));
    let texts = with_forward(group_texts(480));
    let before = render(log.history(), &texts);
    heard(
        &mut log,
        MEMBER,
        "周六见",
        r#"{"msg":"7000","name":"阿杰","ambient":true,"backfill":true}"#,
    );
    let after = render(log.history(), &texts);
    assert_eq!(after, before, "后来记下的不改前面的");
    assert_eq!(render(log.history(), &texts), after, "渲染两次一样");
    assert_eq!(shape(&after), [format!("user: {UNSEEN}")]);
}

/// 判官看的记录同一个规矩：要判的那一条日志里没有被引用的写在下面；有的照旧写在编号后面；之前的记录里的行照它以前的日志判。
#[test]
fn judged_records_follow_the_same_rule() {
    let mut log = Log::new();
    heard(
        &mut log,
        MEMBER,
        "对",
        &quoting("8811", "7000").replace(r#""mentions_me":true,"#, r#""ambient":true,"#),
    );
    heard(
        &mut log,
        MEMBER,
        "周六见",
        r#"{"msg":"7001","name":"阿杰","ambient":true}"#,
    );
    let judged = log.push(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{}}}"#,
            text_json("好"),
            quoting("8812", "7002")
        ),
    );
    let seen = log.push(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{}}}"#,
            text_json("嗯"),
            quoting("8813", "7001")
        ),
    );
    let texts = with_forward(group_texts(480));
    let chat = texts.group.clone().expect("群会话");
    let at = |seq| Seq::new(seq).expect("合写法");
    let got = records(log.history(), at(judged), 20, &chat, texts.forward.as_ref()).expect("有");
    assert_eq!(
        got.records,
        format!(
            "{}\n[15:00] 阿杰 [msg=7001]: 周六见\n",
            UNSEEN.replace("\n  @mentions: [you]", "")
        )
    );
    assert_eq!(
        got.current,
        UNSEEN
            .replace("8811", "8812")
            .replace("7000", "7002")
            .replace("对", "好")
    );
    let got = records(log.history(), at(seen), 20, &chat, texts.forward.as_ref()).expect("有");
    assert_eq!(
        got.current,
        concat!(
            "[15:00] 小林 [msg=8813]: 嗯\n",
            "  reply-to: msg=7001 阿杰: 周六见\n",
            "  @mentions: [you]",
        ),
        "日志里有的照旧写在编号后面"
    );
}
