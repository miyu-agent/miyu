//! 同一个编号的命令再来：落盘前后再来；拒绝过的再来；落盘到一半；落盘超出追加过的；只记最近 1024 个。

use super::super::recent::CAPACITY;
use super::*;

#[test]
fn a_command_sent_again_before_it_is_stored_is_answered_twice_after() {
    let mut session = session();
    session.handle(send(1, "hi"));
    assert!(session.handle(send(1, "hi")).is_empty());
    let actions = session.handle(stored(2));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(1, &[2]), &accepted_reply(1, &[2])]
    );
}

#[test]
fn a_command_sent_again_after_it_is_stored_is_answered_on_the_spot() {
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(2));
    assert_eq!(session.handle(send(1, "hi")), [accepted_reply(1, &[2])]);
}

#[test]
fn a_rejected_command_is_judged_again() {
    let mut session = session();
    let rejected = |actions: Vec<Action>| {
        matches!(
            actions.as_slice(),
            [Action::Reply {
                outcome: Outcome::Rejected { .. },
                ..
            }]
        )
    };
    assert!(rejected(session.handle(send(1, ""))));
    assert!(rejected(session.handle(send(1, ""))));
    assert_eq!(
        appended(&session.handle(send(1, "这回有字了"))).first(),
        Some(&seq(2))
    );
}

#[test]
fn a_partial_store_answers_only_the_commands_fully_stored() {
    let mut session = session();
    session.handle(send(1, "one"));
    // 回合正在开，第二条是中途来的，排在回合开头那几条后面。
    assert_eq!(appended(&session.handle(send(2, "two"))), seqs(&[6]));
    let first = session.handle(stored(2));
    assert!(
        matches!(first.as_slice(), [Action::Push(events), reply] if events.len() == 1 && *reply == accepted_reply(1, &[2]))
    );
    assert!(replies(&session.handle(stored(5))).is_empty());
    let last = session.handle(stored(6));
    assert!(
        matches!(last.as_slice(), [Action::Push(events), reply] if events.len() == 1 && *reply == accepted_reply(2, &[6]))
    );
}

#[test]
fn a_store_beyond_what_was_appended_counts_only_what_was_appended() {
    let mut session = session();
    session.handle(send(1, "one"));
    let actions = session.handle(stored(100));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(1, &[2])],
        "追加过的都落了盘"
    );
    // 之后追加的，要等它自己落了盘。
    session.handle(send(2, "two"));
    assert!(session.handle(send(2, "two")).is_empty());
    assert!(
        session.handle(stored(5)).is_empty(),
        "不比上一次往后的，什么都不做"
    );
    assert_eq!(replies(&session.handle(stored(6))).len(), 2);
}

#[test]
fn only_the_latest_commands_are_remembered() {
    let mut session = session();
    for n in 1..=CAPACITY as u64 + 1 {
        session.handle(send(n, "hi"));
    }
    // 第 1 个命令的消息是 2 号，后面跟着回合开头的三条；第 n 个（n 大于 1）是 n + 4 号。
    let last = CAPACITY as u64 + 5;
    session.handle(stored(last));
    // 第 2 个还记得，照上一次回应；第 1 个已经忘了，当新命令。
    assert_eq!(session.handle(send(2, "hi")), [accepted_reply(2, &[6])]);
    assert_eq!(appended(&session.handle(send(1, "hi"))), seqs(&[last + 1]));
}
