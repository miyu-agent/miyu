//! 会话的上下文上限（施工 CTX-1，设计 68，`configure.rs`）：换了的记一条、只写 `context`；和现在一样的当场回应、什么都不记；
//! `null` 去掉；换模型不清它；载入照整份日志算回来，撤掉的回合里换的也算；给头看的限额带上它。另有压缩线算得出的最小窗口
//! （`Compaction::least_window`）和套上限（`Limits::capped`）。

use super::limits::{COMPACTION, compacting, hand};
use super::load::{Logged, load};
use super::revert::revert;
use super::*;
use crate::event::{ContextCap, PolicyChanged};
use crate::session::Compaction;

/// 编号是 `n` 的命令：主人把会话的上下文上限换成 `tokens`（`None` 是去掉）。
fn set(n: u64, tokens: Option<u64>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::SetContext { tokens },
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

/// 换成 `tokens` 的那一条的 `body`。
fn to(tokens: Option<u64>) -> Body {
    Body::PolicyChanged(PolicyChanged {
        context: Some(ContextCap { tokens }),
        ..PolicyChanged::default()
    })
}

#[test]
fn a_change_is_recorded_once_and_null_takes_it_off() {
    let mut session = session();
    assert_eq!(session.context(), None, "没设的照模型的窗口");
    let actions = session.handle(set(2, Some(128_000)));
    let events = appended_events(&actions);
    assert_eq!(events[0].body, to(Some(128_000)));
    assert_eq!((&events[0].by, &events[0].cause), (&person(), &Some(id(2))));
    session.handle(stored(2));
    assert_eq!(session.context(), Some(128_000));
    assert_eq!(
        session.handle(set(3, Some(128_000))),
        [accepted_reply(3, &[])],
        "一样的不记"
    );
    session.handle(configure(4, "b/n"));
    session.handle(stored(3));
    assert_eq!(session.context(), Some(128_000), "换模型不清上限");
    let actions = session.handle(set(5, None));
    assert_eq!(
        appended_events(&actions)[0].body,
        to(None),
        "null 照样记一条"
    );
    assert_eq!(session.context(), None, "去掉了");
    session.handle(stored(4));
    assert_eq!(
        session.handle(set(6, None)),
        [accepted_reply(6, &[])],
        "本来就没设的什么都不记"
    );
}

#[test]
fn loading_counts_every_change_even_in_undone_turns() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.handle(set(20, Some(200_000)));
    logged.say(seen, "好");
    let last = logged.last();
    logged.handle(revert(21, 3));
    logged.handle(stored(last + 1));
    assert_eq!(
        logged.session.context(),
        Some(200_000),
        "撤掉的回合里换的也算"
    );
    let (mut loaded, _) = load(logged.log.clone());
    assert_eq!(loaded.context(), Some(200_000), "载入照日志算回来");
    assert_eq!(
        loaded.handle(set(30, Some(200_000))),
        [accepted_reply(30, &[])]
    );
}

/// 给头看的限额带上会话的上限；窗口照交来的（套上限是路由的事），压缩线照它算。
#[test]
fn the_limits_shown_carry_the_cap() {
    let mut session = compacting();
    hand(&mut session, Some(128_000), None);
    assert_eq!(session.context_limits().context, None, "没设的不写");
    session.handle(set(2, Some(128_000)));
    let shown = session.context_limits();
    assert_eq!(
        (shown.window, shown.compaction_line, shown.context),
        (Some(128_000), Some(101_600), Some(128_000)),
        "预留 20000、余量 6400 减下来 101600，比窗口的 85% 108800 小"
    );
}

/// 压缩线算得出的最小窗口：和 `line` 同一个算法，正好卡在算得出、算不出的界上。
#[test]
fn the_least_window_is_where_the_line_starts() {
    assert_eq!(
        COMPACTION.least_window(None),
        Some(21_053),
        "没报最大输出：预留 20000，21052 减完正好是 0"
    );
    assert_eq!(
        COMPACTION.least_window(Some(4_000)),
        Some(4_211),
        "预留 4000，余量是窗口的 5%"
    );
    for max_output in [None, Some(1), Some(4_000), Some(8_192), Some(393_216)] {
        let least = COMPACTION.least_window(max_output).expect("算得出");
        assert!(COMPACTION.line(Some(least), max_output).is_some());
        assert_eq!(COMPACTION.line(Some(least - 1), max_output), None);
    }
    let never = Compaction {
        line_percent: 0,
        ..COMPACTION
    };
    assert_eq!(
        never.least_window(None),
        None,
        "压缩线封成 0 的怎么都算不出"
    );
}

/// 内核说的最少要多少：照策略、交过的限额里的最大输出；策略里没有压缩的没有。
#[test]
fn the_least_context_follows_the_policy_and_the_max_output() {
    let mut session = compacting();
    assert_eq!(session.least_context(), Some(21_053), "没交过限额");
    hand(&mut session, Some(1_000_000), Some(4_000));
    assert_eq!(session.least_context(), Some(4_211));
    assert_eq!(super::session().least_context(), None, "没有压缩");
}

/// 套上限：取小的，模型没报窗口的照上限，没设的不动。
#[test]
fn capping_takes_the_smaller_window() {
    let limits = |window| Limits {
        model: crate::testkit::model(),
        window,
        max_output: None,
        images: None,
        blind: false,
        identity: None,
    };
    let capped = |window, context| limits(window).capped(context).window;
    assert_eq!(capped(Some(1_000_000), Some(128_000)), Some(128_000));
    assert_eq!(capped(Some(64_000), Some(128_000)), Some(64_000));
    assert_eq!(capped(None, Some(128_000)), Some(128_000));
    assert_eq!(capped(Some(64_000), None), Some(64_000));
    assert_eq!(capped(None, None), None);
}
