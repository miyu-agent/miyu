//! 场景：respond 带的那一句（施工 RT-1，`Request::tail`，core1 11-07 定）：开出来的这一轮每次请求都带，算完指纹才放（工具第二步
//! 照旧是前缀延伸、不算断缓存），messages 里没有它，下一轮没有；同一份剧本不带它跑，每次请求去掉它一字不差。正在跑的一轮来的
//! respond 带了的换掉、没带的不动，记在 `turn.joined` 上；这一轮没再请求就结束、由并进来的几条接着开的那一轮带上它；被有计划的
//! 重启打断、接着干的那一轮照样带。

use super::*;
use crate::event::{TurnJoined, TurnStarted};

const TAIL: &str = "（群里的提示）";

/// 旁听一句、照它开一轮（带 `tail`）：调一次工具再答；接着人自己说一句开第二轮。
fn run(tail: Option<&str>) -> Stage {
    let mut s = stage();
    s.model([
        Line::calls("我看一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("看完了。"),
        Line::says("嗯。"),
    ]);
    s.tools([Play::done("内容")]);
    let heard = s.overhear("帮我看看 a");
    s.respond(&[heard], tail);
    s.say("还有吗");
    s
}

fn started(s: &Stage) -> Vec<TurnStarted> {
    s.log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::TurnStarted(started) => Some(started.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_request_of_the_responded_turn_carries_it_and_the_next_turn_does_not() {
    let s = run(Some(TAIL));
    let requests = s.requests();
    assert_eq!(requests.len(), 3, "这一轮两步，下一轮一步");
    assert_eq!(requests[0].1.tail.as_deref(), Some(TAIL));
    assert_eq!(
        requests[1].1.tail.as_deref(),
        Some(TAIL),
        "工具第二步照样带"
    );
    assert_eq!(requests[2].1.tail, None, "下一轮没有");
    for (_, request) in requests {
        assert!(
            !format!("{:?}", request.messages).contains(TAIL),
            "不进 messages：{request:?}"
        );
    }
    let turns = started(&s);
    assert_eq!(turns[0].tail.as_deref(), Some(TAIL), "记在 turn.started 上");
    assert_eq!(turns[1].tail, None);
    // 不算进指纹：第二步、下一轮都是前缀延伸，不记一次断缓存。
    let calls = s.model_calls();
    assert!(calls[1].first_difference.is_none(), "{:?}", calls[1]);
    assert!(calls[2].first_difference.is_none(), "{:?}", calls[2]);
}

#[test]
fn without_it_the_same_script_sends_the_same_bytes() {
    let with = run(Some(TAIL));
    let without = run(None);
    assert_eq!(with.requests().len(), without.requests().len());
    for ((_, a), (_, b)) in with.requests().iter().zip(without.requests()) {
        let mut a = a.clone();
        a.tail = None;
        assert_eq!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "去掉那一句一字不差"
        );
        assert_eq!(a.fingerprint(), b.fingerprint());
    }
    // 规范字节带它才写：带了的多出那一格，不带的和以前一样。
    let first = &with.requests()[0].1;
    let bytes = String::from_utf8(first.canonical_bytes()).unwrap();
    assert!(
        bytes.ends_with(&format!(",\"tail\":\"{TAIL}\"}}")),
        "{bytes}"
    );
    let plain = String::from_utf8(without.requests()[0].1.canonical_bytes()).unwrap();
    assert!(!plain.contains("\"tail\""), "{plain}");
}

#[test]
fn joining_with_one_replaces_it_and_without_one_keeps_it() {
    let mut s = stage();
    s.model([
        Line::says("嗯").held(),
        Line::says("再想想").held(),
        Line::says("好。"),
    ]);
    let a = s.overhear("第一句");
    s.respond(&[a], Some("一"));
    let b = s.overhear("第二句");
    s.respond(&[b], Some("二"));
    s.release_model();
    let c = s.overhear("第三句");
    s.respond(&[c], None);
    s.release_model();
    let tails: Vec<Option<&str>> = s
        .requests()
        .iter()
        .map(|(_, request)| request.tail.as_deref())
        .collect();
    assert_eq!(tails, [Some("一"), Some("二"), Some("二")], "没带的不抹掉");
    let joined: Vec<TurnJoined> = s
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::TurnJoined(joined) => Some(joined.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(joined.len(), 2);
    assert_eq!(joined[0].tail.as_deref(), Some("二"), "记在 turn.joined 上");
    assert_eq!(joined[1].tail, None);
    // 每次回复都没再调工具：这一轮没再请求就结束，由并进来的那几条接着开一轮（施工 O-14 下），带上这一轮那一句。
    let opened: Vec<Option<String>> = started(&s).into_iter().map(|turn| turn.tail).collect();
    assert_eq!(
        opened,
        [
            Some("一".to_string()),
            Some("二".to_string()),
            Some("二".to_string())
        ],
        "接着开的那一轮记在它的 turn.started 上：重启以后照样带"
    );
}

#[test]
fn the_turn_resumed_after_a_restart_still_carries_it() {
    let mut s = stage();
    s.model([Line::says("想一下").held(), Line::says("接着说。")]);
    let a = s.overhear("第一句");
    s.respond(&[a], Some(TAIL));
    s.restart();
    let last = s.requests().last().unwrap();
    assert_eq!(last.1.tail.as_deref(), Some(TAIL), "接着干的那一轮照样带");
    let turns = started(&s);
    assert_eq!(turns.len(), 2, "重启以后接着开了一轮");
    assert_eq!(
        turns[1].tail.as_deref(),
        Some(TAIL),
        "写进接着干的那条 turn.started"
    );
}

#[test]
fn a_tail_brought_by_joining_survives_a_restart() {
    let mut s = stage();
    s.model([Line::says("想一下").held(), Line::says("接着说。")]);
    let a = s.overhear("第一句");
    s.respond(&[a], Some("一"));
    let b = s.overhear("第二句");
    s.respond(&[b], Some("二"));
    s.restart();
    assert_eq!(s.requests().last().unwrap().1.tail.as_deref(), Some("二"));
}
