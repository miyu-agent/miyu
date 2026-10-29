//! 中途换人格、预设（施工 P-13，`recast.rs`）：空闲时收，记一条人换的 `session.policy_changed`，接着单开一轮压缩（同手动
//! 压缩），两条落了盘才回应；没有能压的只记那一条。和现在一样的格去掉，一格都不剩的当场回应、什么都不记。有回合在进行的
//! 拒绝。换成的照 [`Session::persona`]、[`Session::preset`] 交给会话那一层，空的人格是无人格；载入照日志算回来。

use super::compact::{after_one_turn, compaction, loaded, one_turn_log, rejected_reply};
use super::load::Logged;
use super::*;
use crate::event::{PolicyChanged, TurnStarted};

/// 编号是 `n` 的命令：主人换人格 `persona`（外层没有是不换）、预设 `preset`。
fn recast(n: u64, persona: Option<Option<&str>>, preset: Option<&str>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::Recast {
            persona: persona.map(|persona| persona.map(str::to_string)),
            preset: preset.map(str::to_string),
        },
    })
}

#[test]
fn a_change_is_recorded_then_compacted_and_answered_once_both_are_stored() {
    let mut session = after_one_turn();
    let actions = session.handle(recast(3, Some(Some("miyu")), Some("chat")));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9, 10]));
    assert_eq!(
        events[0].body,
        Body::PolicyChanged(PolicyChanged {
            persona: Some("miyu".to_string()),
            preset: Some("chat".to_string()),
            ..PolicyChanged::default()
        })
    );
    assert_eq!((&events[0].by, &events[0].cause), (&person(), &Some(id(3))));
    assert!(
        matches!(
            &events[1].body,
            Body::TurnStarted(TurnStarted { trigger: None, .. })
        ),
        "单开一轮压缩：{:?}",
        events[1].body
    );
    assert_eq!(
        (session.persona(), session.preset()),
        (Some("miyu"), Some("chat"))
    );
    assert!(
        replies(&session.handle(stored(9))).is_empty(),
        "两条都落了盘才回应"
    );
    let actions = session.handle(stored(10));
    assert_eq!(replies(&actions), [&accepted_reply(3, &[9, 10])]);
    assert!(hooks(&actions).is_empty(), "压缩那一轮不跑回合开始的挂接点");
    assert_eq!(calls(&actions).len(), 1, "发了摘要请求");
}

#[test]
fn nothing_to_compact_records_only_the_change() {
    let mut session = loaded(one_turn_log(), Some(compaction(1000)), Some(100_000));
    let actions = session.handle(recast(3, Some(Some("miyu")), None));
    assert_eq!(appended(&actions), seqs(&[9]));
    assert_eq!(
        replies(&session.handle(stored(9))),
        [&accepted_reply(3, &[9])]
    );
    assert!(session.idle(), "没开那一轮");
    let actions = session.handle(recast(4, Some(None), None));
    assert_eq!(
        appended_events(&actions)[0].body,
        Body::PolicyChanged(PolicyChanged {
            persona: Some(String::new()),
            ..PolicyChanged::default()
        }),
        "换成无人格写空的"
    );
    assert_eq!(session.persona(), None);
}

#[test]
fn the_same_values_are_answered_at_once_and_a_running_turn_refuses() {
    let mut fresh = after_one_turn();
    assert_eq!(fresh.persona(), None, "日志里造会话时没写人格");
    assert_eq!(
        fresh.handle(recast(4, Some(None), None)),
        [accepted_reply(4, &[])],
        "和现在一样：什么都不记"
    );
    fresh.handle(send(5, "again"));
    assert_eq!(
        fresh.handle(recast(6, Some(Some("miyu")), None)),
        [rejected_reply(6, Reason::TurnRunning)]
    );
    assert_eq!(fresh.persona(), None);
}

#[test]
fn a_loaded_session_remembers_what_it_was_switched_to() {
    let mut logged = Logged::new();
    let seen = logged.ask(2, "hi");
    logged.say(seen, "好。");
    let mut session = loaded(logged.log.clone(), Some(compaction(1000)), Some(100_000));
    let actions = session.handle(recast(3, Some(Some("miyu")), Some("chat")));
    let mut log = logged.log;
    log.extend(appended_events(&actions));
    let again = loaded(log, Some(compaction(1000)), Some(100_000));
    assert_eq!(
        (again.persona(), again.preset()),
        (Some("miyu"), Some("chat"))
    );
}
