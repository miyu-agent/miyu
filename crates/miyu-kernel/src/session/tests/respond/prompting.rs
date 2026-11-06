//! 没有 `to`、照几块事实开一轮（施工 O-64，通讯平台到点看一眼私聊要的）：闲着的先记下事实、不带回合编号，由最后一块开一轮；
//! 正在跑一轮的照样并进去，`turn.joined` 的 `triggers` 是空的；一块事实都没有的拒。

use super::*;
use crate::event::TurnJoined;

/// 编号是 `n` 的命令：桥不写 `to`，带 `facts` 那几块 `glance` 类的事实。
fn glance(n: u64, facts: &[&str]) -> Input {
    Input::Command(Received {
        id: id(n),
        by: bridge(),
        at: at(n % 60),
        command: Command::Respond {
            to: Vec::new(),
            facts: facts
                .iter()
                .map(|text| ContextInjected {
                    kind: FactKind::parse("glance").unwrap(),
                    text: text.to_string(),
                    refs: Vec::new(),
                    after: false,
                })
                .collect(),
            tail: None,
        },
    })
}

#[test]
fn facts_alone_open_a_turn_on_the_last_one() {
    let mut logged = overheard_twice();
    let actions = logged.handle(glance(5, &["<a/>", "<glance/>"]));
    let events = appended_events(&actions);
    assert_eq!(
        events[..2]
            .iter()
            .map(|event| (event.by.clone(), event.turn, event.cause.clone()))
            .collect::<Vec<_>>(),
        [(bridge(), None, Some(id(5))), (bridge(), None, Some(id(5)))],
        "事实先记下，不带回合编号"
    );
    assert_eq!(fact_of(&events[1]).text, "<glance/>");
    assert_eq!(
        events[2].body,
        Body::TurnStarted(TurnStarted {
            trigger: Some(events[1].seq),
            triggers: Vec::new(),
            cwd: Some("~/src/miyu".to_string()),
            dirs: Vec::new(),
            tail: None,
        }),
        "由最后一块开"
    );
    assert_eq!(events[2].cause, Some(id(5)));
    let stored_actions = logged.handle(stored(logged.last()));
    assert!(
        stored_actions
            .iter()
            .any(|action| matches!(action, Action::RunTurnStartHooks { .. })),
        "{stored_actions:?}"
    );
    assert_eq!(
        outcome(&stored_actions),
        Some(&Outcome::Accepted {
            events: events.iter().map(|event| event.seq).collect()
        })
    );
}

#[test]
fn no_messages_and_no_facts_is_refused() {
    let mut logged = overheard_twice();
    let actions = logged.handle(glance(5, &[]));
    assert_eq!(actions, [rejected(id(5), Reason::EmptyMessage)]);
}

#[test]
fn facts_alone_join_a_running_turn_with_no_triggers() {
    let mut logged = overheard_twice();
    let seen = logged.ask(4, "hi");
    let actions = logged.handle(glance(9, &["<glance/>"]));
    let events = appended_events(&actions);
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(events[0].by, bridge());
    assert_eq!(
        events[1].body,
        Body::TurnJoined(TurnJoined {
            triggers: Vec::new(),
            tail: None,
        })
    );
    logged.handle(stored(logged.last()));
    // 这一步说完就结束：她没听到，接着开一轮，由 `turn.joined` 触发。
    logged.say(seen, "嗯");
    let next = logged
        .log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::TurnStarted(started) => Some(started.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(next.trigger, Some(events[1].seq));
}
