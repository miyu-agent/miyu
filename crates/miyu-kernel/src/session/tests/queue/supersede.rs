//! 撤掉重发（施工 4-15，`supersede.rs`）：请求在路上、一个字都还没回时来了一条消息，撤掉这次请求、记一条结果是
//! `superseded` 的 `model.called`，落了盘带上它重发；重发的不算步数；撤掉的那次迟到的增量、说完了都不理；已经开始回字的
//! 照旧排着队。

use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::CallResult;

#[test]
fn a_message_before_any_word_withdraws_the_request_and_sends_it_again() {
    let mut session = asking();
    allowing(&mut session, sent(5));
    let actions = allowing(&mut session, send(2, "顺便看下 tests"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6, 7]), "消息、撤掉的那次请求");
    let called = called_of(&events[1]);
    assert_eq!(
        (called.seen, &called.result, called.usage, &called.error),
        (seq(5), &CallResult::Superseded, None, &None)
    );
    assert_eq!(called.model, Some(deepseek().model), "发出去了的记发给了谁");
    assert_eq!(
        (
            events[1].by.clone(),
            events[1].cause.clone(),
            events[1].turn
        ),
        (By::Kernel, Some(id(2)), Some(turn3())),
        "撤掉它的是这条消息"
    );
    assert_eq!(
        actions.last(),
        Some(&Action::CancelModel { seen: seq(5) }),
        "叫执行器别再发，排在追加后面"
    );
    assert!(calls(&actions).is_empty(), "落了盘才重发");
    // 落了盘：照有效历史重发，那句话在里面。
    let calls = calls(&allowing(&mut session, stored(7)));
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, seq(7));
    assert!(calls[0].1.contains("6 message.user"));
    // 说完了结束这一轮：那句话听到过了，不再接着开。
    let actions = answer(&mut session, 7, "好");
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8, 9, 10]), "回复、记录、结束");
    assert_eq!(reason_of(&events[2]), &EndReason::Completed);
}

#[test]
fn a_request_not_yet_sent_is_withdrawn_too() {
    let mut session = asking();
    let events = appended_events(&allowing(&mut session, send(2, "等等")));
    let called = called_of(&events[1]);
    assert_eq!(called.result, CallResult::Superseded);
    assert_eq!(
        (called.endpoint.clone(), called.model.clone()),
        (None, None)
    );
}

#[test]
fn once_she_has_started_the_message_waits_in_the_queue() {
    let mut session = asking();
    speaking(&mut session, 5);
    let actions = allowing(&mut session, send(2, "顺便看下 tests"));
    assert_eq!(appended(&actions), seqs(&[6]), "只记下消息");
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::CancelModel { .. }))
    );
}

#[test]
fn late_words_of_the_withdrawn_request_are_ignored() {
    let mut session = asking();
    allowing(&mut session, sent(5));
    allowing(&mut session, send(2, "等等"));
    let start = Delta::Start {
        index: 0,
        kind: Kind::Text,
    };
    assert!(session.handle(delta(5, 41, start)).is_empty());
    assert!(session.handle(ended(5)).is_empty());
    // 重发的那次照常收。
    allowing(&mut session, stored(7));
    let events = appended_events(&answer(&mut session, 7, "好"));
    assert_eq!(called_of(&events[1]).result, CallResult::Ok);
}

#[test]
fn several_messages_before_any_word_withdraw_each_request() {
    let mut session = asking();
    allowing(&mut session, send(2, "一"));
    allowing(&mut session, stored(7));
    let actions = allowing(&mut session, send(3, "二"));
    assert_eq!(appended(&actions), seqs(&[8, 9]));
    assert_eq!(called_of(&appended_events(&actions)[1]).seen, seq(7));
    let calls = calls(&allowing(&mut session, stored(9)));
    assert!(calls[0].1.contains("6 message.user") && calls[0].1.contains("8 message.user"));
}

/// 撤掉重发的那一次不算步数：上限 2 步，撤掉一次以后调了工具，还有一步。
#[test]
fn the_resend_does_not_count_as_a_step() {
    let mut policy = policy();
    policy.step_limit = Some(2);
    let mut session = session_with(policy);
    allowing(&mut session, send(1, "hi"));
    allowing(&mut session, stored(5));
    allowing(&mut session, hooks_done(turn3(), Vec::new()));
    allowing(&mut session, send(2, "只看 .rs"));
    allowing(&mut session, stored(7));
    call_tools(&mut session, 7, &[("read", "{}")]);
    allowing(&mut session, stored(9));
    let actions = allowing(&mut session, done(call(8, 1), "a"));
    assert_eq!(appended(&actions), seqs(&[10]), "工具的结果；没到上限");
    assert_eq!(
        calls(&allowing(&mut session, stored(10))).len(),
        1,
        "还有一步"
    );
}

/// 请求在路上时切成了只读，还没回字就来了一句话：撤掉的这一批里照查一遍事实，重发的请求里权限那一块是只读。
#[test]
fn the_facts_are_checked_again_before_the_resend() {
    use super::super::permission::read_only;
    let mut session = asking();
    allowing(&mut session, sent(5));
    assert_eq!(
        appended(&allowing(&mut session, read_only(2, true))),
        seqs(&[6])
    );
    let actions = allowing(&mut session, send(3, "只看看"));
    let events = appended_events(&actions);
    assert_eq!(
        appended(&actions),
        seqs(&[7, 8, 9]),
        "消息、撤掉的请求、权限那一块"
    );
    assert_eq!(fact_of(&events[2]).text, r#"<p l="read_only"/>"#);
    let calls = calls(&allowing(&mut session, stored(9)));
    assert!(calls[0].1.contains("9 context.injected"), "{}", calls[0].1);
}

/// 撤掉重发带进请求的那句话（施工 4-15 补）：重发的请求还在路上就以「马上发」打断，结束这一轮以后接着开一轮回答它，由它
/// 触发；退回、留着的不开（那句话照旧在日志里）。以前打断以后就没人回了：它已经不在队里。
#[test]
fn interrupting_the_request_that_took_a_message_answers_it_next() {
    let opened = |queued: Queued| {
        let mut session = asking();
        allowing(&mut session, sent(5));
        allowing(&mut session, send(2, "你直接输出就行"));
        allowing(&mut session, stored(7));
        allowing(&mut session, sent(7));
        let actions = allowing(&mut session, stop_with(3, at(50), queued));
        appended_events(&actions)
            .iter()
            .find_map(|event| match &event.body {
                Body::TurnStarted(started) => Some((started.trigger, event.cause.clone())),
                _ => None,
            })
    };
    assert_eq!(
        opened(Queued::Send),
        Some((Some(seq(6)), Some(id(2)))),
        "接着开一轮，由那句话触发"
    );
    assert_eq!(opened(Queued::Keep), None, "留着的不开");
}

/// 带着那句话的请求出错了、等着重发时打断（施工 4-15 补）：一样接着开一轮回答它。请求说完了的不算欠着：之后在跑工具时
/// 打断，不再为它开一轮。
#[test]
fn a_message_still_unanswered_after_an_error_is_answered_after_the_interruption() {
    let mut session = asking();
    allowing(&mut session, sent(5));
    allowing(&mut session, send(2, "你直接输出就行"));
    allowing(&mut session, stored(7));
    allowing(&mut session, sent(7));
    allowing(
        &mut session,
        failed(7, crate::event::ErrorClass::Retryable, "connection reset"),
    );
    let actions = allowing(&mut session, stop_with(3, at(50), Queued::Send));
    let started = appended_events(&actions)
        .iter()
        .find_map(|event| match &event.body {
            Body::TurnStarted(started) => started.trigger,
            _ => None,
        });
    assert_eq!(started, Some(seq(6)), "等着重发的也接着开");

    let mut session = asking();
    allowing(&mut session, sent(5));
    allowing(&mut session, send(2, "你直接输出就行"));
    allowing(&mut session, stored(7));
    allowing(&mut session, sent(7));
    call_tools(&mut session, 7, &[("read", "{}")]);
    let actions = allowing(&mut session, stop_with(3, at(50), Queued::Send));
    assert!(
        !appended_events(&actions)
            .iter()
            .any(|event| matches!(event.body, Body::TurnStarted(_))),
        "说完了的算回答过"
    );
}
