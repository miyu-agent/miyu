//! 会话钉的看图模型（施工 VM-1，`configure.rs`）：换了的记一条、只写 `vision`；和现在一样的当场回应、什么都不记；`None`
//! 记成空的、去掉；换模型不清它；载入照整份日志算回来，撤掉的回合里换的也算。

use super::load::{Logged, load};
use super::revert::revert;
use super::*;
use crate::event::PolicyChanged;

/// 编号是 `n` 的命令：主人把会话的看图模型钉成 `vision`（`None` 是去掉）。
fn set(n: u64, vision: Option<&str>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::SetVision {
            vision: vision.map(str::to_string),
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

/// 钉成 `vision` 的那一条的 `body`：去掉的是空的。
fn to(vision: &str) -> Body {
    Body::PolicyChanged(PolicyChanged {
        vision: Some(vision.to_string()),
        ..PolicyChanged::default()
    })
}

#[test]
fn a_pin_is_recorded_once_and_none_takes_it_off() {
    let mut session = session();
    assert_eq!(session.vision(), None, "没钉的照 models.vision");
    let actions = session.handle(set(2, Some("b/v")));
    let events = appended_events(&actions);
    assert_eq!(events[0].body, to("b/v"));
    assert_eq!((&events[0].by, &events[0].cause), (&person(), &Some(id(2))));
    session.handle(stored(2));
    assert_eq!(session.vision(), Some("b/v"));
    assert_eq!(
        session.handle(set(3, Some("b/v"))),
        [accepted_reply(3, &[])],
        "一样的不记"
    );
    session.handle(configure(4, "b/n"));
    session.handle(stored(3));
    assert_eq!(session.vision(), Some("b/v"), "换模型不清它");
    let actions = session.handle(set(5, None));
    assert_eq!(appended_events(&actions)[0].body, to(""), "去掉记成空的");
    assert_eq!(session.vision(), None, "去掉了");
    session.handle(stored(4));
    assert_eq!(
        session.handle(set(6, None)),
        [accepted_reply(6, &[])],
        "本来就没钉的什么都不记"
    );
}

#[test]
fn loading_counts_every_pin_even_in_undone_turns() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.handle(set(20, Some("@eyes")));
    logged.say(seen, "好");
    let last = logged.last();
    logged.handle(revert(21, 3));
    logged.handle(stored(last + 1));
    assert_eq!(
        logged.session.vision(),
        Some("@eyes"),
        "撤掉的回合里换的也算"
    );
    let (mut loaded, _) = load(logged.log.clone());
    assert_eq!(loaded.vision(), Some("@eyes"), "载入照日志算回来");
    assert_eq!(
        loaded.handle(set(30, Some("@eyes"))),
        [accepted_reply(30, &[])]
    );
}
