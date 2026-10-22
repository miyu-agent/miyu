//! 不打断，退回一条排着的（施工 Q-W，`session.withdraw`）：退的是要的那一条（不写的是最新的），回合照跑，回复完了由剩下的
//! 接着开；不在队里的拒绝 `already_heard`，没写、队里是空的拒绝 `nothing_queued`。

use super::super::compact::rejected_reply;
use super::*;

/// 在说最后的回答时补了两句：6、7 号排着队。
fn two_queued() -> Session {
    let mut session = asking();
    speaking(&mut session, 5);
    allowing(&mut session, send(2, "顺便把 README 也看了"));
    allowing(&mut session, send(4, "还有 Cargo.toml"));
    session
}

#[test]
fn withdrawing_takes_back_the_latest_and_the_turn_goes_on() {
    let mut session = two_queued();
    let actions = allowing(&mut session, withdraw(5, None));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8]), "只记撤回，不结束这一轮");
    assert_eq!(withdrawn(&events[0]).messages, seqs(&[7]));
    assert_eq!(
        (
            events[0].by.clone(),
            events[0].cause.clone(),
            events[0].turn
        ),
        (alice(), Some(id(5)), Some(turn3()))
    );
    assert!(
        session.handle(stored(8)).contains(&accepted_reply(5, &[8])),
        "落了盘回应"
    );
    // 回复完了，剩下的 6 号接着开一轮。
    let actions = answer_from(&mut session, 5, 1, "好");
    let events = appended_events(&actions);
    let last = events.last().expect("接着开了一轮");
    assert_eq!(started(last).1, seq(6));
}

#[test]
fn withdrawing_a_named_one_leaves_the_rest() {
    let mut session = two_queued();
    let actions = allowing(&mut session, withdraw(5, Some(6)));
    assert_eq!(
        withdrawn(&appended_events(&actions)[0]).messages,
        seqs(&[6])
    );
    allowing(&mut session, stored(8));
    let actions = answer_from(&mut session, 5, 1, "好");
    let last = appended_events(&actions).pop().expect("接着开了一轮");
    assert_eq!(started(&last).1, seq(7));
}

#[test]
fn what_is_not_queued_cannot_be_withdrawn() {
    let mut session = two_queued();
    // 开这一轮的那一句：她听到了。
    assert_eq!(
        session.handle(withdraw(5, Some(2))),
        vec![rejected_reply(5, Reason::AlreadyHeard)]
    );
    allowing(&mut session, withdraw(6, Some(7)));
    // 退过的再退。
    assert_eq!(
        session.handle(withdraw(7, Some(7))),
        vec![rejected_reply(7, Reason::AlreadyHeard)]
    );
    allowing(&mut session, withdraw(8, None));
    assert_eq!(
        session.handle(withdraw(9, None)),
        vec![rejected_reply(9, Reason::NothingQueued)]
    );
    // 闲着的会话：没有排着的。
    assert_eq!(
        super::super::session().handle(withdraw(1, None)),
        vec![rejected_reply(1, Reason::NothingQueued)]
    );
}
