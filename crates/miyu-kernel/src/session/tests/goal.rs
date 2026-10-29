//! 改长期目标的命令（施工 X-7，`session/goal.rs`）：五种各记对应的那一条 `goal.*`，落了盘回应；不合的当场拒：已经有一个没
//! 结束的再设、没有的改停接着跑结束、停着的再停、跑着的接着跑、原文空白。卡住的照停着办（施工 X-8 下）。

use std::num::NonZeroU32;

use super::*;
use crate::event::GoalOutcome;
use crate::session::GoalChange;

/// 编号是 `n` 的改目标的命令，主人发的。
fn goal(n: u64, change: GoalChange) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::Goal(change),
    })
}

fn set(text: &str) -> GoalChange {
    GoalChange::Set {
        text: text.to_string(),
        limit: NonZeroU32::new(5),
    }
}

/// 送进一条，交回追加的那一条的种类；拒了的交回原因码。
fn kind_or_reason(session: &mut Session, input: Input) -> String {
    let actions = session.handle(input);
    match appended_events(&actions).first() {
        Some(event) => event.body.kind().to_string(),
        None => replies(&actions)
            .iter()
            .find_map(|action| match action {
                Action::Reply {
                    outcome: Outcome::Rejected { reason, .. },
                    ..
                } => Some(reason.code().to_string()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("既没记也没拒：{actions:?}")),
    }
}

#[test]
fn each_change_records_its_event_and_wrong_ones_are_refused() {
    let mut s = session();
    let steps = [
        (goal(1, GoalChange::Pause), "no_goal"),
        (goal(2, set("  ")), "empty_message"),
        (goal(3, set("修绿")), "goal.set"),
        (goal(4, set("别的")), "goal_exists"),
        (goal(5, GoalChange::Resume), "goal_running"),
        (
            goal(
                6,
                GoalChange::Change {
                    text: "修绿再写文档".to_string(),
                    limit: None,
                },
            ),
            "goal.changed",
        ),
        (goal(7, GoalChange::Pause), "goal.paused"),
        (goal(8, GoalChange::Pause), "goal_paused"),
        (goal(9, GoalChange::Resume), "goal.resumed"),
        (
            goal(
                10,
                GoalChange::End {
                    outcome: GoalOutcome::Blocked,
                    reason: Some("要人给 key".to_string()),
                },
            ),
            "goal.ended",
        ),
        // 卡住的照停着办（施工 X-8 下）：还算没完成，再设拒、停拒，接着跑收。
        (goal(11, set("下一个")), "goal_exists"),
        (goal(12, GoalChange::Pause), "goal_paused"),
        (goal(13, GoalChange::Resume), "goal.resumed"),
        (
            goal(
                14,
                GoalChange::End {
                    outcome: GoalOutcome::Cleared,
                    reason: None,
                },
            ),
            "goal.ended",
        ),
        (goal(15, GoalChange::Resume), "no_goal"),
        (goal(16, set("下一个")), "goal.set"),
    ];
    for (input, wanted) in steps {
        assert_eq!(kind_or_reason(&mut s, input), wanted);
        let last = s.ledger.next_seq().get() - 1;
        s.handle(stored(last));
    }
}

/// 载入时还跑着的目标记成停着（施工 X-7）：`by` 内核、没有 `cause`；停着的、没有目标的不记。
#[test]
fn loading_pauses_a_running_goal() {
    let mut logged = super::load::Logged::new();
    logged.handle(goal(1, set("修绿")));
    logged.handle(stored(logged.last()));
    let (_, actions) = super::load::load(logged.log.clone());
    let appended = appended_events(&actions);
    assert_eq!(appended.len(), 1, "{appended:?}");
    assert!(matches!(appended[0].body, Body::GoalPaused(_)));
    assert_eq!((&appended[0].by, &appended[0].cause), (&By::Kernel, &None));
    logged.handle(goal(2, GoalChange::Pause));
    logged.handle(stored(logged.last()));
    let (_, actions) = super::load::load(logged.log.clone());
    assert!(appended_events(&actions).is_empty(), "停着的不再记");
}
