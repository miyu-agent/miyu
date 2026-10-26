//! 长期目标现在的样子（施工 X-6，`docs/blueprint/goal.md`「在新的架构里怎么放」第 1、7 条）：照日志算，同累计用量一个喂法：
//! 载入时照整份日志算（`open/load.rs`），之后每落一批盘加上这一批（`actor/store.rs`）。头上看到的会话状态的 `goal` 照它拼
//! （端点的 `view/status.rs`）。续轮、授权、等人开口随施工 X-7：那几样看的是这次运行里的事，不在日志里。

use std::num::NonZeroU32;

use miyu_kernel::event::{Body, Event, GoalOutcome};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use crate::current::Tally;

/// 一个目标现在的样子。
#[derive(Debug, Clone, PartialEq)]
pub struct Goal {
    /// 原文，改过的照改完的。
    pub text: String,
    /// 轮数上限；不限的没有。
    pub limit: Option<NonZeroU32>,
    /// 跑着、停着、完成、卡住。
    pub state: GoalState,
    /// 卡在哪：卡住的才有。
    pub reason: Option<String>,
    /// 设下以后开过几轮：停着时开的不算。
    pub round: u32,
    /// 停过以后又接着跑了几次（施工 GOAL-1）：照 `goal.resumed` 数，人续上的、改停着的顺带续上的、核心重启载入记成停着以后续上的都算。
    pub resumes: u32,
    /// 设下的时刻。
    pub since: Timestamp,
    /// 她自己用 `goal` 工具设的（施工 X-8 上，`by` 是一次工具调用）：续轮消息照它说是谁设的。
    pub by_you: bool,
    /// 花了多少：跑着时的请求，口径同会话累计。
    pub spent: Tally,
}

/// 目标在哪一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalState {
    /// 跑着。
    Running,
    /// 停着。
    Paused,
    /// 她报做完了。
    Completed,
    /// 她报卡住了，或者到了轮数上限。
    Blocked,
    /// 跑着，在等人开口：续轮开的一轮没调工具（施工 X-7，这次运行里的事，日志里没有：会话 actor 给头看时盖上）。
    Waiting,
}

impl GoalState {
    /// 在协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            GoalState::Running => "running",
            GoalState::Paused => "paused",
            GoalState::Completed => "completed",
            GoalState::Blocked => "blocked",
            GoalState::Waiting => "waiting",
        }
    }
}

/// 从日志一条条算的目标：没有设过、人清掉了的没有；结束了的照最后的样子留着，再设一个时换掉。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Goals {
    now: Option<Goal>,
}

impl Goals {
    /// 照 `events` 从头算（测试里核对一批批加的和它一样；载入时一路加，施工 V-15）。
    #[cfg(test)]
    pub(crate) fn of(events: &[Event]) -> Goals {
        let mut goals = Goals::default();
        goals.add(events);
        goals
    }

    /// 现在的目标。
    pub fn now(&self) -> Option<&Goal> {
        self.now.as_ref()
    }

    /// 加上 `events` 这一批。
    pub(crate) fn add(&mut self, events: &[Event]) {
        for event in events {
            self.note(event);
        }
    }

    /// 记下一条。
    fn note(&mut self, event: &Event) {
        if let Body::GoalSet(set) = &event.body {
            self.now = Some(Goal {
                text: set.text.clone(),
                limit: set.limit,
                state: GoalState::Running,
                reason: None,
                round: 0,
                resumes: 0,
                since: event.at,
                by_you: matches!(event.by, By::Tool(_)),
                spent: Tally::default(),
            });
            return;
        }
        let Some(goal) = self.now.as_mut() else {
            return;
        };
        match &event.body {
            Body::GoalChanged(changed) => {
                goal.text.clone_from(&changed.text);
                goal.limit = changed.limit;
            }
            Body::GoalPaused(_) => goal.state = GoalState::Paused,
            // 卡住的接着跑（施工 X-8 下）：原因不再给人看。
            Body::GoalResumed(_) => {
                goal.state = GoalState::Running;
                goal.reason = None;
                goal.resumes += 1;
            }
            Body::GoalEnded(ended) => match &ended.outcome {
                GoalOutcome::Completed => goal.state = GoalState::Completed,
                GoalOutcome::Blocked => {
                    goal.state = GoalState::Blocked;
                    goal.reason.clone_from(&ended.reason);
                }
                // 清掉的、不认识的结局都不再给人看。
                GoalOutcome::Cleared | GoalOutcome::Other(_) => self.now = None,
            },
            Body::TurnStarted(_) if goal.state == GoalState::Running => goal.round += 1,
            Body::ModelCalled(_) if goal.state == GoalState::Running => {
                goal.spent.add(std::slice::from_ref(event));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
