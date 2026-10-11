//! 改长期目标（施工 X-7，`docs/blueprint/goal.md`）：`/goal` 命令、`goal` 工具、到了轮数上限都交这一个命令。照账本里现在的目标
//! 判（`Ledger::goal_running`），合的记对应的那一条 `goal.*`，回合进行中的带上这个回合；不合的拒。不开轮：续轮由会话 actor 开
//! （`miyu-session` 的 `actor/goal.rs`）。载入时还跑着的记成停着（[`Session::pause_goal_on_load`]）：核心重启以后不自己接着跑，
//! 要人 `/goal resume`。

use super::action::{Action, Reason};
use super::input::GoalChange;
use super::{Session, rejected};
use crate::event::{Body, GoalChanged, GoalEnded, GoalPaused, GoalResumed, GoalSet};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 改长期目标：已经有一个没结束的再设 `goal_exists`；没有的改、停、接着跑、结束 `no_goal`；停着的再停 `goal_paused`，跑着
    /// 的接着跑 `goal_running`；原文是空白的 `empty_message`。合的落了盘回应，附上那一条的序号。
    pub(super) fn goal(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        change: GoalChange,
    ) -> Vec<Action> {
        let now = self.ledger.goal_running();
        let body = match (change, now) {
            (GoalChange::Set { .. }, Some(_)) => return vec![rejected(id, Reason::GoalExists)],
            (GoalChange::Set { text, limit }, None) => {
                if text.trim().is_empty() {
                    return vec![rejected(id, Reason::EmptyMessage)];
                }
                Body::GoalSet(GoalSet { text, limit })
            }
            (_, None) => return vec![rejected(id, Reason::NoGoal)],
            (GoalChange::Change { text, limit }, Some(_)) => {
                if text.trim().is_empty() {
                    return vec![rejected(id, Reason::EmptyMessage)];
                }
                Body::GoalChanged(GoalChanged { text, limit })
            }
            (GoalChange::Pause, Some(false)) => return vec![rejected(id, Reason::GoalPaused)],
            (GoalChange::Pause, Some(true)) => Body::GoalPaused(GoalPaused {}),
            (GoalChange::Resume, Some(true)) => return vec![rejected(id, Reason::GoalRunning)],
            (GoalChange::Resume, Some(false)) => Body::GoalResumed(GoalResumed {}),
            (GoalChange::End { outcome, reason }, Some(_)) => {
                Body::GoalEnded(GoalEnded { outcome, reason })
            }
        };
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 载入以后：目标还跑着的，记一条 `goal.paused`（`by` 内核，没有 `cause`）。交回追加的那一条。
    pub(super) fn pause_goal_on_load(&mut self, at: Timestamp) -> Option<crate::event::Event> {
        (self.ledger.goal_running() == Some(true))
            .then(|| self.record(at, By::Kernel, None, Body::GoalPaused(GoalPaused {})))
    }
}
