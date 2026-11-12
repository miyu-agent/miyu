//! 确认到时没人确认（施工 CD-1，`docs/blueprint/kernel/asking.md`「到时没人确认」）：这一轮配了等多久的，请求带截止的时刻、
//! 交出到时叫醒；到时还在等的，内核替人拒绝，记拒绝的决定和被拒绝的结果，`by` 都是内核，这一步照常往下走；人先定了的、来了
//! 一句话作废了的，到时不理；没配的一直等。

use super::*;

/// 截止的时刻：链在 48 秒交回结论（[`guarded`]），等 300 秒。
const DEADLINE: &str = "2026-09-25T07:05:48.000Z";

/// 这一轮等人确认最多等 `ms` 毫秒；请求 5 号调一件写文件的，链说要问人（请求是 8 号，落了盘）。交回会话和链交回结论那一步的
/// 动作。
fn waiting_with(ms: Option<u64>) -> (Session, Vec<Action>) {
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    let mut hooked = hooks_done(turn3(), Vec::new());
    if let Input::TurnStartHooksDone { confirm_ms, .. } = &mut hooked {
        *confirm_ms = ms;
    }
    session.handle(hooked);
    call_tools(&mut session, 5, &[("write", "{}")]);
    session.handle(stored(7));
    let actions = session.handle(guarded(call(6, 1), ask(Access::Write, true)));
    session.handle(stored(8));
    (session, actions)
}

/// 到时了：照执行器交的样子，那一句是 `timed out`。
fn expired(call_id: CallId) -> Input {
    Input::ApprovalExpired {
        at: Timestamp::parse(DEADLINE).unwrap(),
        call_id,
        nested: None,
        text: "timed out".to_string(),
        human: Some(Said::new("core/tool-results/approval-timed-out")),
    }
}

#[test]
fn a_request_carries_its_deadline_and_asks_to_be_woken_then() {
    let (_, actions) = waiting_with(Some(300_000));
    let events = appended_events(&actions);
    let deadline = Timestamp::parse(DEADLINE).unwrap();
    assert_eq!(request_of(&events[0]).expires_at, Some(deadline));
    assert!(
        actions.contains(&Action::ExpireApproval {
            at: deadline,
            call_id: call(6, 1),
            nested: None,
        }),
        "{actions:?}"
    );
}

#[test]
fn without_a_wait_it_waits_forever() {
    let (_, actions) = waiting_with(None);
    let events = appended_events(&actions);
    assert_eq!(request_of(&events[0]).expires_at, None);
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::ExpireApproval { .. })),
        "{actions:?}"
    );
}

#[test]
fn when_time_is_up_the_kernel_denies_it_and_she_goes_on() {
    let (mut session, _) = waiting_with(Some(300_000));
    let actions = session.handle(expired(call(6, 1)));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9, 10]));
    let decided = decision_of(&events[0]);
    assert_eq!(
        (decided.call_id, &decided.decision, &decided.reason),
        (call(6, 1), &Decision::Deny, &None)
    );
    assert_eq!(
        (events[0].by.clone(), events[0].cause.clone()),
        (By::Kernel, Some(id(1))),
        "内核替人拒的，cause 是回合的"
    );
    assert_eq!(
        result_of(&events[1]),
        (
            call(6, 1),
            ToolStatus::Denied,
            By::Kernel,
            "timed out".to_string()
        )
    );
    assert_eq!(
        said_of(&events[1]),
        Some(Said::new("core/tool-results/approval-timed-out"))
    );
    assert_eq!(events[1].cause, Some(id(1)));
    // 她接着干：这一步齐了，请求下一次。
    let actions = session.handle(stored(10));
    assert_eq!(
        calls(&actions).first().map(|(seen, _)| *seen),
        Some(seq(10))
    );
    // 到时以后人才答的：已经不在等了。
    assert_eq!(
        session.handle(answer(2, call(6, 1), Decision::Once, None)),
        rejected_with(Reason::NotAsking, 2)
    );
}

#[test]
fn a_decision_before_the_deadline_makes_it_moot() {
    for decision in [Decision::Once, Decision::Deny] {
        let (mut session, _) = waiting_with(Some(300_000));
        session.handle(answer(2, call(6, 1), decision.clone(), None));
        session.handle(stored(10));
        assert!(
            session.handle(expired(call(6, 1))).is_empty(),
            "{decision:?} 以后到时了不理"
        );
    }
}

#[test]
fn a_message_that_voids_the_request_voids_the_deadline_too() {
    let (mut session, _) = waiting_with(Some(300_000));
    let actions = session.handle(send(2, "换个办法"));
    let events = appended_events(&actions);
    assert!(
        events.iter().any(|event| matches!(
            &event.body,
            Body::ToolResult(result) if result.status == ToolStatus::Skipped
        )),
        "来了一句话，在等的跳过：{events:?}"
    );
    session.handle(stored(11));
    assert!(session.handle(expired(call(6, 1))).is_empty());
}

#[test]
fn a_deadline_for_a_call_that_never_asked_is_ignored() {
    let (mut session, _) = waiting_with(Some(300_000));
    assert!(session.handle(expired(call(6, 2))).is_empty());
    assert!(session.handle(expired(call(7, 1))).is_empty());
}
