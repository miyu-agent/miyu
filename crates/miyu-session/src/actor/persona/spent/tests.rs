//! 回放日志摊累计用量（施工 CX-3）：主请求才算（辅助请求、摘要请求不算；被打断没报用量的算次数、不摊）；每次照它看到的
//! 那一刻的对话；换了策略的到下一个回合开始才换快照；一份快照只备一次。

use std::cell::RefCell;

use miyu_kernel::event::Event;
use miyu_kernel::id::ContentHash;
use miyu_kernel::session::{Frame, Said};

use super::{Prepared, replay};
use crate::SystemPart;

const ONE: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const TWO: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const PERSON: &str = r#"{"kind":"person","account":"alice"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;

fn event(seq: u64, by: &str, kind: &str, body: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-10-23T07:00:00.000Z","kind":"{kind}","by":{by},"body":{body}}}"#
    ))
    .unwrap()
}

fn said(seq: u64, by: &str, kind: &str, text: &str) -> Event {
    event(
        seq,
        by,
        kind,
        &format!(r#"{{"blocks":[{{"type":"text","text":"{text}"}}]}}"#),
    )
}

/// 一次请求：看到第 `seen` 条；`input` 是报的输入（没有就是没报），输出 10；`extra` 是别的格。
fn called(seq: u64, seen: u64, input: Option<u64>, extra: &str) -> Event {
    let usage = input
        .map(|n| {
            format!(r#""usage":{{"uncached":{n},"cache_read":0,"cache_write":0,"output":10}},"#)
        })
        .unwrap_or_default();
    event(
        seq,
        KERNEL,
        "model.called",
        &format!(
            r#"{{"seen":{seen},"endpoint":"deepseek","model":"deepseek-flash","messages":1,{usage}{extra}"result":"ok"}}"#
        ),
    )
}

/// 真日志的先后：回复先落盘，`model.called` 跟在后面，`seen` 指回发请求时看到的那一条。
fn log() -> Vec<Event> {
    let reply = |seq: u64, seen: u64| {
        event(
            seq,
            KERNEL,
            "message.assistant",
            &format!(r#"{{"blocks":[{{"type":"text","text":"好的好的好的"}}],"seen":{seen}}}"#),
        )
    };
    vec![
        event(
            1,
            PERSON,
            "session.created",
            &format!(
                r#"{{"owner":"alice","venue":"local","policy":"{ONE}","permission":{{"level":"workspace","read_only":false}}}}"#
            ),
        ),
        event(2, KERNEL, "turn.started", "{}"),
        said(3, PERSON, "message.user", "一二三四五六七八"),
        reply(4, 3),
        called(5, 3, Some(100), ""),
        // 回顾（辅助请求）、摘要请求：不算。
        called(6, 5, Some(50), r#""purpose":"recap","#),
        called(7, 5, Some(70), r#""compaction":"manual","#),
        event(
            8,
            PERSON,
            "session.policy_changed",
            &format!(r#"{{"policy":"{TWO}"}}"#),
        ),
        // 换了策略还没到下一个回合：照旧。
        said(9, PERSON, "message.user", "再说一句"),
        reply(10, 9),
        called(11, 9, Some(200), ""),
        event(12, KERNEL, "turn.started", "{}"),
        said(13, PERSON, "message.user", "第三句"),
        // 被打断、没报用量：算一次、不摊。
        called(14, 13, None, ""),
        reply(15, 13),
        called(16, 13, Some(300), ""),
    ]
}

fn prepared(system: u64) -> Prepared {
    Prepared {
        frame: Frame {
            system,
            tools: vec![("read".to_string(), 5)],
            demos: 0,
            price: miyu_kernel::estimate::Flat { image: 0, file: 0 },
        },
        system: vec![(SystemPart::Persona, system)],
        offered: Vec::new(),
    }
}

#[test]
fn main_requests_are_charged_with_the_context_they_saw() {
    let asked = RefCell::new(Vec::new());
    let spent = replay(log(), |hash: &ContentHash| {
        asked.borrow_mut().push(hash.to_string());
        Some(match hash.to_string().as_str() {
            ONE => prepared(40),
            _ => prepared(80),
        })
    });
    assert_eq!(spent.requests, 4, "主请求四次，辅助、摘要不算");
    assert_eq!(spent.output, 30, "报了用量的三次主请求的输出");
    let inputs: Vec<u64> = spent.charged.iter().map(|charged| charged.input).collect();
    assert_eq!(inputs, [100, 200, 300], "没报用量的不摊");
    let systems: Vec<u64> = spent
        .charged
        .iter()
        .map(|charged| charged.context.breakdown.system)
        .collect();
    assert_eq!(systems, [40, 40, 80], "换的策略下一个回合才生效");
    assert_eq!(*asked.borrow(), [ONE, TWO], "一份快照只备一次");
    let user = |at: usize| {
        spent.charged[at]
            .context
            .breakdown
            .said
            .iter()
            .find(|(said, _)| *said == Said::User)
            .map(|(_, tokens)| *tokens)
    };
    let first = miyu_kernel::estimate::text("一二三四五六七八");
    assert_eq!(user(0), Some(first), "第一次只看到第一句");
    assert_eq!(
        user(1),
        Some(first + miyu_kernel::estimate::text("再说一句")),
        "第二次连第二句"
    );
    assert!(
        spent.charged[0]
            .context
            .breakdown
            .said
            .iter()
            .all(|(said, _)| *said != Said::Assistant),
        "第一次请求时还没有回复"
    );
}
