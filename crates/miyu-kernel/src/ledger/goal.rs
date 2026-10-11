//! 长期目标的几条规矩（施工 X-6，`docs/blueprint/goal.md`）：同一时刻至多一个没结束的目标；没有目标的改不了、停不了、接着跑
//! 不了、结束不了；跑着的才停得了，停着的才接着跑得了；原文不能是空白。卡住的（施工 X-8 下）照停着办：接着跑、改、结束都收，
//! 停拒；再设一个也收（X-8 上的日志里卡住以后她再设过，只放宽不收紧），命令那一层另拒（`session/goal.rs`）。

use super::Ledger;
use crate::event::{Body, Event, GoalOutcome};

/// 现在的目标：没有、跑着、停着、卡住。完成了、清掉了的算没有。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GoalStage {
    /// 没有，或者完成了、清掉了。
    #[default]
    None,
    /// 跑着。
    Running,
    /// 停着。
    Paused,
    /// 卡住了（施工 X-8 下）：人答了能接着跑。
    Blocked,
}

impl Ledger {
    /// 现在的长期目标（施工 X-7，X-8 下起分出卡住的）。
    pub fn goal(&self) -> GoalStage {
        self.goals
    }
}

impl GoalStage {
    /// 照目标的规矩查 `event`。
    pub(super) fn check(self, event: &Event) -> Result<(), String> {
        let blank = |text: &str| match text.trim().is_empty() {
            true => Err("a goal needs some text".to_string()),
            false => Ok(()),
        };
        match (&event.body, self) {
            (Body::GoalSet(_), GoalStage::Running | GoalStage::Paused) => {
                Err("a goal is already set; change or end it first".to_string())
            }
            (Body::GoalSet(set), GoalStage::None | GoalStage::Blocked) => blank(&set.text),
            (
                Body::GoalChanged(_)
                | Body::GoalPaused(_)
                | Body::GoalResumed(_)
                | Body::GoalEnded(_),
                GoalStage::None,
            ) => Err(format!(
                "{} needs a goal that has not ended",
                event.body.kind()
            )),
            (Body::GoalChanged(changed), _) => blank(&changed.text),
            (Body::GoalPaused(_), GoalStage::Paused) => {
                Err("the goal is already paused".to_string())
            }
            (Body::GoalPaused(_), GoalStage::Blocked) => Err("the goal is blocked".to_string()),
            (Body::GoalResumed(_), GoalStage::Running) => Err("the goal is not paused".to_string()),
            _ => Ok(()),
        }
    }

    /// 记下追加的一条。
    pub(super) fn record(&mut self, event: &Event) {
        *self = match (&event.body, *self) {
            (Body::GoalSet(_) | Body::GoalResumed(_), _) => GoalStage::Running,
            (Body::GoalPaused(_), _) => GoalStage::Paused,
            (Body::GoalEnded(ended), _) if ended.outcome == GoalOutcome::Blocked => {
                GoalStage::Blocked
            }
            // 不认识的结局也算结束了（`GoalOutcome` 的说明）。
            (Body::GoalEnded(_), _) => GoalStage::None,
            (_, now) => now,
        };
    }
}
