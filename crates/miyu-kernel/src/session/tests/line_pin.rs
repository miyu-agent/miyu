//! 压缩触发线可配（施工 CT-1，`configure.rs`、`policy.rs`）：压缩线照不同的百分数算，减法那一条照旧封顶；会话钉的换了的记一条、
//! 只写 `compaction_line`，和现在一样的当场回应、什么都不记，`None` 去掉；换模型不清它；载入照整份日志算回来，撤掉的回合里换的也算。

use super::limits::COMPACTION;
use super::load::{Logged, load};
use super::revert::revert;
use super::*;
use crate::event::{LinePin, PolicyChanged};
use crate::session::Compaction;

/// 出厂的数，只把压缩线的百分数换成 `percent`。
fn at_percent(percent: u64) -> Compaction {
    Compaction {
        line_percent: percent,
        ..COMPACTION
    }
}

/// 20 万、百万、小窗口，各照 20、85、95 三档：窗口的 N% 和「窗口 − 输出预留 − 余量」取小的。
#[test]
fn the_line_follows_the_percent_until_the_subtraction_caps_it() {
    let cases = [
        // 20 万、没报最大输出：预留 20000，余量 10000，减下来 170000。85 以上都是它。
        (200_000, None, [40_000, 170_000, 170_000]),
        // 百万：余量 13000，减下来 967000，95 也到得了。
        (1_000_000, None, [200_000, 850_000, 950_000]),
        // 小窗口、最大输出 4000：预留 4000，余量 1600，减下来 26400。
        (32_000, Some(4_000), [6_400, 26_400, 26_400]),
    ];
    for (window, max_output, lines) in cases {
        for (percent, line) in [20, 85, 95].into_iter().zip(lines) {
            assert_eq!(
                at_percent(percent).line(Some(window), max_output),
                Some(line),
                "窗口 {window}、{percent}%"
            );
        }
    }
    assert_eq!(
        at_percent(90).line(Some(200_000), None),
        Some(170_000),
        "20 万窗口调到 85 以上不起作用"
    );
}

/// 编号是 `n` 的命令：主人把会话的压缩触发线钉成 `percent`（`None` 是去掉）。
fn set(n: u64, percent: Option<u64>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::SetCompactionLine { percent },
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

/// 钉成 `percent` 的那一条的 `body`。
fn to(percent: Option<u64>) -> Body {
    Body::PolicyChanged(PolicyChanged {
        compaction_line: Some(LinePin { percent }),
        ..PolicyChanged::default()
    })
}

#[test]
fn a_pin_is_recorded_once_and_none_takes_it_off() {
    let mut session = session();
    assert_eq!(session.pinned_line(), None, "没钉的照 compaction.line");
    let actions = session.handle(set(2, Some(50)));
    let events = appended_events(&actions);
    assert_eq!(events[0].body, to(Some(50)));
    assert_eq!((&events[0].by, &events[0].cause), (&person(), &Some(id(2))));
    session.handle(stored(2));
    assert_eq!(session.pinned_line(), Some(50));
    assert_eq!(
        session.handle(set(3, Some(50))),
        [accepted_reply(3, &[])],
        "一样的不记"
    );
    session.handle(configure(4, "b/n"));
    session.handle(stored(3));
    assert_eq!(session.pinned_line(), Some(50), "换模型不清它");
    let actions = session.handle(set(5, None));
    assert_eq!(appended_events(&actions)[0].body, to(None), "去掉记成 null");
    assert_eq!(session.pinned_line(), None, "去掉了");
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
    logged.handle(set(20, Some(60)));
    logged.say(seen, "好");
    let last = logged.last();
    logged.handle(revert(21, 3));
    logged.handle(stored(last + 1));
    assert_eq!(
        logged.session.pinned_line(),
        Some(60),
        "撤掉的回合里换的也算"
    );
    let (mut loaded, _) = load(logged.log.clone());
    assert_eq!(loaded.pinned_line(), Some(60), "载入照日志算回来");
    assert_eq!(loaded.handle(set(30, Some(60))), [accepted_reply(30, &[])]);
}
