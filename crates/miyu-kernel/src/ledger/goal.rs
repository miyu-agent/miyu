//! 长期目标的几条规矩（施工 X-6，`docs/blueprint/goal.md`）：同一时刻至多一个没结束的目标；没有目标的改不了、停不了、接着跑
//! 不了、结束不了；跑着的才停得了，停着的才接着跑得了；原文不能是空白。

use super::Ledger;
use crate::event::{Body, Event};

/// 现在的目标：没有、跑着、停着。结束了的算没有。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Goals {
    #[default]
    None,
    Running,
    Paused,
}

impl Ledger {
    /// 现在的长期目标（施工 X-7）：没有、结束了的是 `None`，跑着的 `Some(true)`，停着的 `Some(false)`。
    pub fn goal_running(&self) -> Option<bool> {
        match self.goals {
            Goals::None => None,
            Goals::Running => Some(true),
            Goals::Paused => Some(false),
        }
    }
}

impl Goals {
    /// 照目标的规矩查 `event`。
    pub(super) fn check(self, event: &Event) -> Result<(), String> {
        let blank = |text: &str| match text.trim().is_empty() {
            true => Err("a goal needs some text".to_string()),
            false => Ok(()),
        };
        match (&event.body, self) {
            (Body::GoalSet(_), Goals::Running | Goals::Paused) => {
                Err("a goal is already set; change or end it first".to_string())
            }
            (Body::GoalSet(set), Goals::None) => blank(&set.text),
            (
                Body::GoalChanged(_)
                | Body::GoalPaused(_)
                | Body::GoalResumed(_)
                | Body::GoalEnded(_),
                Goals::None,
            ) => Err(format!(
                "{} needs a goal that has not ended",
                event.body.kind()
            )),
            (Body::GoalChanged(changed), _) => blank(&changed.text),
            (Body::GoalPaused(_), Goals::Paused) => Err("the goal is already paused".to_string()),
            (Body::GoalResumed(_), Goals::Running) => Err("the goal is not paused".to_string()),
            _ => Ok(()),
        }
    }

    /// 记下追加的一条。
    pub(super) fn record(&mut self, event: &Event) {
        *self = match (&event.body, *self) {
            (Body::GoalSet(_) | Body::GoalResumed(_), _) => Goals::Running,
            (Body::GoalPaused(_), _) => Goals::Paused,
            // 不认识的结局也算结束了（`GoalOutcome` 的说明）。
            (Body::GoalEnded(_), _) => Goals::None,
            (_, now) => now,
        };
    }
}
