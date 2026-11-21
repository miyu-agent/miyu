//! 场所的语音上限（施工 VM-2，`voice.rs`）：造的时候记的算数；桥报的和现在的不一样的记一条、只写 `voice_max_ms`，一样的当场
//! 回应、什么都不记；撤掉的回合里报的也算，载入照整份日志算回来，分叉出来的照分叉点那一刻的。

use super::load::{Logged, load};
use super::revert::revert;
use super::*;
use crate::event::PolicyChanged;
use crate::origin::Module;

/// 报的那个桥。
fn bridge() -> By {
    By::Module(Module {
        id: crate::id::ModuleId::parse("bridge").unwrap(),
    })
}

/// 编号是 `n` 的命令：桥报这个场所一段语音至多 `ms` 毫秒。
fn report(n: u64, ms: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: bridge(),
        at: at(n % 60),
        command: Command::SetVoiceLimit { ms },
    })
}

/// 报成 `ms` 的那一条的 `body`。
fn to(ms: u64) -> Body {
    Body::PolicyChanged(Box::new(PolicyChanged {
        voice_max_ms: Some(ms),
        ..PolicyChanged::default()
    }))
}

#[test]
fn a_new_limit_is_recorded_once_by_the_bridge() {
    let mut session = session();
    assert_eq!(session.voice_max_ms(), None, "造的时候没报");
    let actions = session.handle(report(2, 60_000));
    let events = appended_events(&actions);
    assert_eq!(events[0].body, to(60_000));
    assert_eq!((&events[0].by, &events[0].cause), (&bridge(), &Some(id(2))));
    session.handle(stored(2));
    assert_eq!(session.voice_max_ms(), Some(60_000));
    assert_eq!(
        session.handle(report(3, 60_000)),
        [accepted_reply(3, &[])],
        "一样的不记"
    );
    let actions = session.handle(report(4, 30_000));
    assert_eq!(appended_events(&actions)[0].body, to(30_000), "变了再记");
    assert_eq!(session.voice_max_ms(), Some(30_000));
}

#[test]
fn the_created_limit_counts_and_the_same_report_records_nothing() {
    let mut created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.voice_max_ms = Some(60_000);
    let (mut session, _) = Session::create(
        session_id(),
        id(0),
        person(),
        at(0),
        created,
        policy(),
        environment("~/src/miyu"),
    );
    session.handle(stored(1));
    assert_eq!(session.voice_max_ms(), Some(60_000));
    assert_eq!(session.handle(report(2, 60_000)), [accepted_reply(2, &[])]);
}

#[test]
fn loading_counts_every_report_even_in_undone_turns() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.handle(report(20, 60_000));
    logged.say(seen, "好");
    let last = logged.last();
    logged.handle(revert(21, 3));
    logged.handle(stored(last + 1));
    assert_eq!(
        logged.session.voice_max_ms(),
        Some(60_000),
        "撤掉的回合里报的也算"
    );
    let plan = super::fork::fork_plan(&session_id(), &logged.log).unwrap();
    assert_eq!(plan.voice_max_ms, Some(60_000), "分叉照那一刻的");
    let (mut loaded, _) = load(logged.log.clone());
    assert_eq!(loaded.voice_max_ms(), Some(60_000), "载入照日志算回来");
    assert_eq!(loaded.handle(report(30, 60_000)), [accepted_reply(30, &[])]);
}
