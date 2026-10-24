//! fork 子代理（施工 A-F 上，`session/fork/within.rs`）：分叉点在父会话这一轮派它的那一步上。子会话头几条给那一步的每个调用
//! 记占位结果、以 `forked` 收住那一轮，接着说一句另开它自己的一轮：请求里底子那一段和父会话的一样，后面是占位结果和这一句；
//! 两个兄弟一字不差；载入的和活着的一样；普通分叉照旧退到这一轮之前、照旧拒停在一轮当中的；停的地方不对的拒。

use super::executor::{reason_of, result_of};
use super::load::Logged;
use super::*;
use crate::event::{EndReason, Forked};
use crate::id::CallId;
use crate::session::{fork_plan, fork_plan_at};
use crate::testkit::CHILD_SESSION;

const PLACEHOLDER: &str = "Forked; the parent keeps going.";

fn child_id() -> SessionId {
    SessionId::parse(CHILD_SESSION).unwrap()
}

/// 父会话说「hi」，这一轮第一步调了 `subagent`、`read`，回复落了盘、两件都还没结果。
fn parent() -> Logged {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.tools(seen, &[("subagent", r#"{"prompt":"x"}"#), ("read", "{}")]);
    logged
}

/// 父会话日志里那一步的两个调用。
fn step_calls(log: &[Event]) -> Vec<CallId> {
    log.iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::MessageAssistant(message) => Some(
                message
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::ToolCall(call) => Some(call.call_id),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap()
}

/// 从父会话第 `upto` 条、第 `turn` 轮当中分叉的 `session.created`。
fn created_within(upto: Seq, turn: Option<TurnId>) -> SessionCreated {
    let mut created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.parent = Some(session_id());
    created.depth = Some(1);
    created.forked = Some(Forked {
        from: session_id(),
        upto,
        seal_after: None,
        within: turn,
    });
    created
}

/// 从父会话派第二个调用的那一步分叉，头几条落了盘：交回子会话、它的头几条、底子。
fn fork_at(parent: &Logged, child: SessionId) -> (Session, Vec<Event>, Vec<Event>) {
    let calls = step_calls(&parent.log);
    let plan = fork_plan_at(&session_id(), &parent.log, calls[1]).unwrap();
    let base = parent.upto(plan.upto.get());
    let (mut session, actions) = Session::fork_within(
        child,
        &base,
        PLACEHOLDER,
        id(100),
        alice(),
        at(30),
        created_within(plan.upto, plan.within),
        policy(),
        environment("~/src/miyu"),
    )
    .unwrap();
    let events = appended_events(&actions);
    session.handle(stored(events.last().unwrap().seq.get()));
    (session, events, base)
}

/// 子会话在第 `last` 条之后收到交代「做这件」，交给执行器的那次请求列出来的有效历史。
fn directed(session: &mut Session, last: u64) -> String {
    session.handle(send(2, "做这件"));
    session.handle(stored(last + 2));
    calls(&session.handle(hooks_done(TurnId::new(seq(last + 2)), Vec::new())))
        .remove(0)
        .1
}

#[test]
fn the_plan_forks_at_the_step_that_made_the_call() {
    let parent = parent();
    let calls = step_calls(&parent.log);
    let plan = fork_plan_at(&session_id(), &parent.log, calls[1]).unwrap();
    assert_eq!(plan.upto, seq(6), "派它的那条 message.assistant");
    assert_eq!(plan.within, Some(TurnId::new(seq(3))));
    let plain = fork_plan(&session_id(), &parent.log).unwrap();
    assert_eq!(plain.upto, seq(2), "普通分叉照旧退到这一轮之前");
    assert_eq!(plain.within, None);
    // 第一个调用已经有了结果：不是在等的调用，不认。
    assert!(fork_plan_at(&session_id(), &parent.log, calls[0]).is_err());
    let mut idle = Logged::new();
    let seen = idle.ask(1, "hi");
    idle.say(seen, "好");
    assert!(
        fork_plan_at(&session_id(), &idle.log, calls[1]).is_err(),
        "闲着的没有正在进行的一轮"
    );
}

#[test]
fn a_fork_within_a_turn_settles_that_step_with_placeholders() {
    let parent = parent();
    let calls = step_calls(&parent.log);
    let (_, events, _) = fork_at(&parent, child_id());
    let kinds: Vec<(u64, &str)> = events
        .iter()
        .map(|event| (event.seq.get(), event.body.kind()))
        .collect();
    assert_eq!(
        kinds,
        [
            (7, "session.created"),
            (8, "tool.result"),
            (9, "tool.result"),
            (10, "turn.ended")
        ],
        "接着分叉点往下数"
    );
    let turn = Some(TurnId::new(seq(3)));
    for (event, call) in events[1..3].iter().zip(&calls) {
        assert_eq!(
            result_of(event),
            (*call, ToolStatus::Ok, By::Kernel, PLACEHOLDER.to_string())
        );
        assert_eq!(event.turn, turn, "带接过来的那一轮");
        assert_eq!(event.cause, Some(id(100)));
    }
    assert_eq!(reason_of(&events[3]), &EndReason::Forked);
    assert_eq!(events[3].turn, turn);
}

#[test]
fn the_child_goes_on_from_the_parents_step_and_siblings_match() {
    let parent = parent();
    let (mut child, events, _) = fork_at(&parent, child_id());
    let last = events.last().unwrap().seq.get();
    let listed = directed(&mut child, last);
    let lines: Vec<&str> = listed.lines().collect();
    let parent_lines: Vec<String> = parent
        .upto(6)
        .iter()
        .map(|event| format!("{} {}", event.seq, event.body.kind()))
        .collect();
    assert_eq!(
        lines[..parent_lines.len()],
        parent_lines.iter().map(String::as_str).collect::<Vec<_>>()[..],
        "底子那一段是父会话到派它的那一步为止的：{listed}"
    );
    assert_eq!(
        lines[parent_lines.len()..],
        [
            "7 session.created",
            "8 tool.result",
            "9 tool.result",
            "10 turn.ended",
            "11 message.user",
            "12 turn.started"
        ],
        "后面是占位结果、收住、交代开的一轮：{listed}"
    );
    let sibling_id = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d92").unwrap();
    let (mut sibling, sibling_events, _) = fork_at(&parent, sibling_id);
    assert_eq!(
        format!("{:?}", sibling_events[1..].to_vec()),
        format!("{:?}", events[1..].to_vec()),
        "兄弟 fork 的占位结果一字不差"
    );
    assert_eq!(directed(&mut sibling, last), listed);
}

#[test]
fn a_fork_within_a_turn_loads_back_the_same() {
    let parent = parent();
    let (child, events, base) = fork_at(&parent, child_id());
    let mut logged = Logged {
        session: child,
        log: events,
    };
    let seen = logged.ask(2, "做这件");
    logged.say(seen, "做完了");
    let (mut loaded, actions) = Session::load_forked(
        child_id(),
        &base,
        logged.log.clone(),
        at(55),
        policy(),
        environment("~/src/miyu"),
    )
    .unwrap();
    assert!(
        actions.is_empty(),
        "接过来的那一轮收住了，载入时什么都不补：{actions:?}"
    );
    let last = logged.last();
    loaded.handle(send(3, "再来"));
    logged.session.handle(send(3, "再来"));
    loaded.handle(stored(last + 2));
    logged.session.handle(stored(last + 2));
    let turn = TurnId::new(seq(last + 2));
    assert_eq!(
        calls(&loaded.handle(hooks_done(turn, Vec::new()))),
        calls(&logged.session.handle(hooks_done(turn, Vec::new()))),
        "载入的和活着的接着走一样"
    );
}

#[test]
fn a_fork_point_in_the_wrong_place_is_refused() {
    let parent = parent();
    let fork = |base: &[Event], created: SessionCreated, within: bool| {
        let opened = (child_id(), id(100), alice(), at(30));
        match within {
            true => Session::fork_within(
                opened.0,
                base,
                PLACEHOLDER,
                opened.1,
                opened.2,
                opened.3,
                created,
                policy(),
                environment("~/src/miyu"),
            ),
            false => Session::fork(
                opened.0,
                base,
                opened.1,
                opened.2,
                opened.3,
                created,
                policy(),
                environment("~/src/miyu"),
            ),
        }
        .map(|_| ())
    };
    let turn = Some(TurnId::new(seq(3)));
    let at_step = parent.upto(6);
    assert!(
        fork(&at_step, created_within(seq(6), None), false).is_err(),
        "普通分叉照旧拒停在一轮当中的"
    );
    assert!(
        fork(&at_step, created_within(seq(6), turn), false).is_err(),
        "within 走 fork_within"
    );
    assert!(
        fork(&at_step, created_within(seq(6), None), true).is_err(),
        "fork_within 要写 within"
    );
    let wrong_turn = Some(TurnId::new(seq(2)));
    assert!(
        fork(&at_step, created_within(seq(6), wrong_turn), true).is_err(),
        "不是那一轮"
    );
    let started = parent.upto(5);
    assert!(
        fork(&started, created_within(seq(5), turn), true).is_err(),
        "要停在一步刚派出去的调用上"
    );
    let before = parent.upto(2);
    assert!(
        fork(&before, created_within(seq(2), None), true).is_err(),
        "分叉点在一轮之外的不走 fork_within"
    );
    assert!(fork(&before, created_within(seq(2), None), false).is_ok());
    assert!(fork(&at_step, created_within(seq(6), turn), true).is_ok());
}

/// 子会话自己的回复看到的不能早于接过来的那条回复（`seen` 一次比一次大，账本接着父会话的最近一条回复查）。
#[test]
fn a_childs_reply_must_have_seen_the_step_it_came_from() {
    let parent = parent();
    let (child, events, base) = fork_at(&parent, child_id());
    let mut logged = Logged {
        session: child,
        log: events,
    };
    let seen = logged.ask(2, "做这件");
    logged.say(seen, "做完了");
    let load = |log: Vec<Event>| {
        Session::load_forked(
            child_id(),
            &base,
            log,
            at(55),
            policy(),
            environment("~/src/miyu"),
        )
        .map(|_| ())
    };
    assert!(load(logged.log.clone()).is_ok());
    let mut early = logged.log.clone();
    for event in &mut early {
        if let Body::MessageAssistant(message) = &mut event.body {
            message.seen = seq(5);
        }
    }
    assert!(load(early).is_err(), "看到的在派它的那条回复之前");
}
