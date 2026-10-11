//! `goal` 工具交来的事（施工 X-8 上，`docs/blueprint/tools/goal.md`，`tools/goals.rs`）：看的照会话状态当场答；设、报完成、
//! 报卡住交内核 `Goal` 命令，`by` 是这次调用，记下等它的回应、把等的口子交回工具。续轮开的那一轮里报卡住，目标开过的轮数
//! 不到 [`BLOCKED_AFTER`] 的当场拒（`goal.md`「照旧版留下的」第 3 条：人开的那一轮、人插过话的这一次运行不管，人说停就停）。

use tokio::sync::oneshot;

use miyu_kernel::event::GoalOutcome;
use miyu_kernel::id::{CallId, CommandId};
use miyu_kernel::origin::{By, Tool};
use miyu_kernel::session::{Command, GoalChange, Input, Received};
use miyu_kernel::time::Timestamp;
use miyu_tool::{BLOCKED_AFTER, GoalAsk, GoalNow, GoalRefused, GoalSeen};

use super::super::{Actor, answer};
use crate::goal::{Goal, GoalState};
use crate::tools::GoalBack;

impl Actor {
    /// 第 `call_id` 次调用交来 `ask`：答了的从 `reply` 送回去；要交内核的交回命令。不在跑、叫它停过的调用不理：口子跟着丢。
    pub(in crate::actor) fn goal_asked(
        &mut self,
        at: Timestamp,
        call_id: CallId,
        ask: GoalAsk,
        reply: oneshot::Sender<GoalBack>,
    ) -> Option<Input> {
        if !self.tools.live(call_id) {
            return None;
        }
        let now = self.counted().goals.now().cloned();
        let seen = GoalSeen {
            now: now.as_ref().map(shown),
            in_round: self.goal_run.turn.is_some(),
        };
        let change = match ask {
            GoalAsk::Get => {
                answer(reply, GoalBack::Answered(Ok(seen)));
                return None;
            }
            GoalAsk::Set { text, limit } => GoalChange::Set { text, limit },
            GoalAsk::Complete => GoalChange::End {
                outcome: GoalOutcome::Completed,
                reason: None,
            },
            GoalAsk::Blocked { reason } => {
                let heard = self.goal_run.heard;
                if let Some(goal) = now
                    .as_ref()
                    .filter(|goal| too_early(goal, seen.in_round, heard))
                {
                    let refused = GoalRefused::TooEarly { round: goal.round };
                    answer(reply, GoalBack::Answered(Err(refused)));
                    return None;
                }
                GoalChange::End {
                    outcome: GoalOutcome::Blocked,
                    reason: Some(reason),
                }
            }
        };
        // 编号照调用编号派生：内核照编号只生效一次，重跑的同一次调用不会改两遍。
        let id = CommandId::parse(&format!("goal-{call_id}")).ok()?;
        let (waiting, outcome) = oneshot::channel();
        self.wait_for(id.clone(), waiting);
        answer(reply, GoalBack::Waiting { seen, outcome });
        Some(Input::Command(Received {
            id,
            by: By::Tool(Tool { call_id }),
            at,
            command: Command::Goal(change),
        }))
    }
}

/// 续轮开的那一轮里、跑着的目标开过的轮数不到 [`BLOCKED_AFTER`]、这一次运行里人没插过话（施工 X-9：人答了「照实报卡住」，
/// 她不该再凑轮数）：报卡住太早。
fn too_early(goal: &Goal, in_round: bool, heard: bool) -> bool {
    in_round && !heard && goal.state == GoalState::Running && goal.round < BLOCKED_AFTER
}

/// 给她看的样子。
fn shown(goal: &Goal) -> GoalNow {
    GoalNow {
        text: goal.text.clone(),
        limit: goal.limit,
        state: goal.state.as_str(),
        reason: goal.reason.clone(),
        round: goal.round,
    }
}
