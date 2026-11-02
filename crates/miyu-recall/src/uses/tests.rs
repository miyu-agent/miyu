//! 用到一次（`memory.md` 第八条）：模块 `memory` 注入的一块的 `refs`、她搜到的那几条算，别的不算；载入时只交照到的位置以后的。

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use super::*;

const MEMORY: &str = r#"{"kind":"module","id":"memory"}"#;
const OTHER: &str = r#"{"kind":"module","id":"knowledge"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;

fn line(seq: u64, kind: &str, by: &str, body: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-10-11T08:00:{:02}.000Z","kind":"{kind}","turn":1,"by":{by},"body":{body}}}"#,
        seq % 60
    ))
    .expect("合写法")
}

fn injected(seq: u64, by: &str, refs: &[&str]) -> Event {
    let refs = serde_json::to_string(refs).expect("写得出");
    line(
        seq,
        "context.injected",
        by,
        &format!(r#"{{"kind":"memory","text":"<memories>\n</memories>","refs":{refs}}}"#),
    )
}

fn called(seq: u64, call: &str, name: &str) -> Event {
    line(
        seq,
        "message.assistant",
        MODEL,
        &format!(
            r#"{{"blocks":[{{"type":"tool_call","call_id":"{call}","name":"{name}","args":"{{}}"}}],"seen":{}}}"#,
            seq - 1
        ),
    )
}

fn result(seq: u64, call: &str, status: &str, text: &str) -> Event {
    let text = serde_json::to_string(text).expect("写得出");
    line(
        seq,
        "tool.result",
        &format!(r#"{{"kind":"tool","call_id":"{call}"}}"#),
        &format!(
            r#"{{"call_id":"{call}","status":"{status}","blocks":[{{"type":"text","text":{text}}}]}}"#
        ),
    )
}

fn feed() -> UseFeed {
    UseFeed::new("memory", "memory_search")
}

/// 交回的：哪一条事件、哪一条记忆。
fn pairs(uses: &[Use]) -> Vec<(u64, String)> {
    uses.iter()
        .map(|used| (used.seq.get(), used.memory.to_string()))
        .collect()
}

#[test]
fn what_the_memory_module_brought_counts_once_each() {
    let mut feed = feed();
    let uses = feed.see(&injected(3, MEMORY, &["m12", "m7", "m12", "x"]));
    assert_eq!(
        pairs(&uses),
        [(3, "m12".into()), (3, "m7".into())],
        "一条事件里列了几遍算一次，不是编号的不算"
    );
    assert_eq!(uses[0].at.to_string(), "2026-10-11T08:00:03.000Z");
    assert!(
        feed.see(&injected(4, OTHER, &["m12"])).is_empty(),
        "别的模块的不算"
    );
    assert!(feed.see(&injected(5, KERNEL, &["m12"])).is_empty());
}

#[test]
fn what_her_search_listed_counts_up_to_the_first_line_that_is_not_a_memory() {
    let mut feed = feed();
    assert!(feed.see(&called(4, "call_4_1", "memory_search")).is_empty());
    let text = "m12 user 2026-10-08: 用户养了一只猫\nm3 feedback 2026-10-01 (retired): 回答要短\n2026-10-09 session 0192f3a0: 我的显卡是 N 卡\nm7 user 2026-10-02: 不算";
    let uses = feed.see(&result(5, "call_4_1", "ok", text));
    assert_eq!(pairs(&uses), [(5, "m12".into()), (5, "m3".into())]);
    assert!(
        feed.see(&result(6, "call_4_1", "ok", "m9 user 2026-10-08: 再来一遍"))
            .is_empty(),
        "一次调用只认一个结果"
    );

    feed.see(&called(7, "call_7_1", "memory_search"));
    assert!(
        feed.see(&result(8, "call_7_1", "ok", "Nothing found."))
            .is_empty()
    );
    feed.see(&called(9, "call_9_1", "memory_search"));
    assert!(
        feed.see(&result(10, "call_9_1", "error", "m12 user 2026-10-08: 猫"))
            .is_empty(),
        "没成的不算"
    );
    feed.see(&called(11, "call_11_1", "read"));
    assert!(
        feed.see(&result(12, "call_11_1", "ok", "m12 user 2026-10-08: 猫"))
            .is_empty(),
        "别的工具的不算"
    );
}

#[test]
fn priming_hands_back_only_what_came_after_the_mark() {
    let events = [
        injected(1, MEMORY, &["m1"]),
        called(2, "call_2_1", "memory_search"),
        injected(3, MEMORY, &["m2"]),
        result(4, "call_2_1", "ok", "m5 user 2026-10-08: 猫"),
    ];
    let mut feed = feed();
    let uses = feed.prime(&events, Some(Seq::new(2).expect("从 1 起")));
    assert_eq!(
        pairs(&uses),
        [(3, "m2".into()), (4, "m5".into())],
        "调用在照到的位置以前、结果在以后的照样认得"
    );
    let mut whole = self::feed();
    assert_eq!(whole.prime(&events, None).len(), 3, "一条都没照过的整份交");
}

/// 原来的 `UseFeed::prime`（施工 R-4 下起，V-37 下以前），照抄：喂一条，照这一条的序号挑。
fn old_prime(feed: &mut UseFeed, events: &[Event], after: Option<Seq>) -> Vec<Use> {
    let mut uses = Vec::new();
    for event in events {
        let seen = feed.see(event);
        if after.is_none_or(|after| event.seq > after) {
            uses.extend(seen);
        }
    }
    uses
}

/// 一条条喂、收齐了再照序号挑（施工 V-37 下，[`since`]），和原来喂一条挑一条的一样：照种子造的几千份，注入的、调搜索的、
/// 结果（列了几条、没列、失败的、别的工具的），照到哪落在各处。
#[test]
fn seeing_all_then_keeping_what_came_after_is_the_old_priming() {
    let mut dice = 0x5eed_2026_1103_u64;
    let mut below = |n: u64| {
        dice ^= dice << 13;
        dice ^= dice >> 7;
        dice ^= dice << 17;
        dice % n.max(1)
    };
    for round in 0..3000 {
        let mut events = Vec::new();
        let mut calls = Vec::new();
        for seq in 2..=below(60) + 2 {
            let event = match below(4) {
                0 => injected(
                    seq,
                    [MEMORY, OTHER][below(2) as usize],
                    &["m1", "m2", "m2", "x"][..below(5) as usize],
                ),
                1 => {
                    let call = format!("call_{seq}_1");
                    calls.push(call.clone());
                    called(seq, &call, ["memory_search", "read"][below(2) as usize])
                }
                _ => {
                    let call = match calls.len() {
                        0 => "call_999_1".to_string(),
                        n => calls[below(n as u64) as usize].clone(),
                    };
                    let text = [
                        "m3 user 2026-10-08: 猫\nm4 user 2026-10-08: 狗",
                        "Nothing found.",
                        "m5 user 2026-10-08: 鱼",
                    ][below(3) as usize];
                    result(seq, &call, ["ok", "ok", "error"][below(3) as usize], text)
                }
            };
            events.push(event);
        }
        let after = Seq::new(below(events.len() as u64 + 2));
        let old = old_prime(&mut feed(), &events, after);
        let mut whole = feed();
        assert_eq!(
            pairs(&whole.prime(&events, after)),
            pairs(&old),
            "第 {round} 份"
        );
        let mut one = feed();
        let mut seen = Vec::new();
        events.iter().for_each(|event| seen.extend(one.see(event)));
        assert_eq!(since(seen, after), old, "第 {round} 份");
        assert_eq!(format!("{one:?}"), format!("{whole:?}"), "第 {round} 份");
    }
}
