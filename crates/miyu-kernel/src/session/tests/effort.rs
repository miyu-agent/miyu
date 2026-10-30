//! 换会话的思考强度（施工 SM-1，设计 51，`configure.rs`）：换了的记一条、只写 `effort`；和现在一样的当场回应、什么都不记；
//! `null` 回到跟着配置；换模型不清它；载入照整份日志算回来，撤掉的回合里换的也算；施工 8-18 那时带 `model` 的一格不认。

use super::load::{Logged, load};
use super::revert::revert;
use super::*;
use crate::event::{Effort, PolicyChanged};

/// 编号是 `n` 的命令：主人把会话的思考强度换成 `level`（`None` 是回到跟着配置）。
fn set(n: u64, level: Option<&str>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::SetEffort {
            level: level.map(str::to_string),
        },
    })
}

/// 编号是 `n` 的命令：换模型。
fn configure(n: u64, model: &str) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::Configure {
            model: model.to_string(),
        },
    })
}

/// 换成 `level` 的那一条的 `body`。
fn to(level: Option<&str>) -> Body {
    Body::PolicyChanged(PolicyChanged {
        effort: Some(Effort {
            model: None,
            level: level.map(str::to_string),
        }),
        ..PolicyChanged::default()
    })
}

#[test]
fn a_change_is_recorded_once_and_null_goes_back_to_the_config() {
    let mut session = session();
    assert_eq!(session.effort(), None, "没记的照配置");
    let actions = session.handle(set(2, Some("high")));
    let events = appended_events(&actions);
    assert_eq!(events[0].body, to(Some("high")));
    assert_eq!((&events[0].by, &events[0].cause), (&person(), &Some(id(2))));
    session.handle(stored(2));
    assert_eq!(session.effort(), Some("high"));
    assert_eq!(
        session.handle(set(3, Some("high"))),
        [accepted_reply(3, &[])],
        "一样的不记"
    );
    // 换模型不清它。
    session.handle(configure(4, "b/n"));
    session.handle(stored(3));
    assert_eq!(session.effort(), Some("high"), "换模型不清会话那一档");
    let actions = session.handle(set(5, None));
    assert_eq!(
        appended_events(&actions)[0].body,
        to(None),
        "null 照样记一条"
    );
    assert_eq!(session.effort(), None, "回到跟着配置");
    session.handle(stored(4));
    assert_eq!(
        session.handle(set(6, None)),
        [accepted_reply(6, &[])],
        "本来就跟着的什么都不记"
    );
}

#[test]
fn loading_counts_every_change_even_in_undone_turns_and_skips_the_8_18_cells() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.handle(set(20, Some("max")));
    logged.say(seen, "好");
    let last = logged.last();
    logged.handle(revert(21, 3));
    logged.handle(stored(last + 1));
    assert_eq!(logged.session.effort(), Some("max"), "撤掉的回合里换的也算");
    // 8-18 那时的一格：带 `model`，不算会话的。
    let old = Body::PolicyChanged(PolicyChanged {
        effort: Some(Effort {
            model: Some("a/m".to_string()),
            level: Some("low".to_string()),
        }),
        ..PolicyChanged::default()
    });
    let mut events = logged.log.clone();
    let mut tail = events.last().cloned().expect("有日志");
    tail.seq = seq(tail.seq.get() + 1);
    tail.body = old;
    tail.turn = None;
    tail.by = person();
    tail.cause = Some(id(29));
    events.push(tail);
    let (mut loaded, _) = load(events);
    assert_eq!(
        loaded.effort(),
        Some("max"),
        "载入照日志算回来，8-18 的不认"
    );
    assert_eq!(
        loaded.handle(set(30, Some("max"))),
        [accepted_reply(30, &[])]
    );
}
