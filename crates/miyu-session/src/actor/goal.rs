//! 长期目标的续轮（施工 X-7，`docs/blueprint/goal.md`「照旧版留下的」第 1、2、4、5、6 条，「在新的架构里怎么放」第 2、3 条）：
//! actor 每办完一封信回到主循环时看一眼（`actor.rs` 的 `run`），目标跑着、会话空着、收件箱是空的（人优先）、没在等人开口，交内核一个说话的命令，
//! `by` 是模块 `goal`，正文是续轮消息；到了轮数上限的改成记卡住。授权就是「跑着」：载入时内核把还跑着的记成停着（核心重启
//! 以后要人 `/goal resume`），跑着的一定是这次运行里设下、接着跑的。等人开口只在内存里，照这次运行里落了盘的事件记
//! （[`GoalRun::note`]）。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, GoalOutcome, round_command};
use miyu_kernel::id::{CommandId, ModuleId, Seq, TurnId};
use miyu_kernel::origin::{By, Module};
use miyu_kernel::session::{Command, GoalChange, Input, Received};

use super::Actor;
use crate::Pushed;
use crate::TARGET;
use crate::goal::{Goal, GoalState};

/// 发续轮的模块。
const MODULE: &str = "goal";

/// 这次运行里的续轮账。
#[derive(Debug, Default)]
pub(super) struct GoalRun {
    /// 在等人开口：续轮开的一轮一件工具都没调就结束了；人说一句话就不等了。
    waiting: bool,
    /// 完整版发过了：设、接着跑、改了以后的第一轮发完整版，之后发短版。
    full_sent: bool,
    /// 续轮消息落盘的序号：它开的那一轮是续轮。
    ours: Option<Seq>,
    /// 续轮开的那一轮：回合编号、调没调过工具。
    turn: Option<(TurnId, bool)>,
    /// 上一次发的时候目标在第几轮：同一轮不发两次（被拒了、没开起来的不再发）。
    sent_at: Option<u32>,
    /// 上一次推给头的样子（施工 X-8 补）：变了才推。
    pushed: Option<Goal>,
    /// 这一次运行里人插过话（施工 X-9）：说过话、答过她的题。报卡住不再受三轮下限（人说停随时停）；设下、接着跑时重新算。
    heard: bool,
}

impl GoalRun {
    /// 记下落了盘的这一批。
    pub(super) fn note(&mut self, events: &[Event]) {
        for event in events {
            match &event.body {
                Body::GoalSet(_) | Body::GoalResumed(_) => {
                    self.waiting = false;
                    self.full_sent = false;
                    self.sent_at = None;
                    self.heard = false;
                }
                Body::GoalChanged(_) => self.full_sent = false,
                Body::MessageUser(_) if is_ours(&event.by) => self.ours = Some(event.seq),
                Body::MessageUser(_) if matches!(event.by, By::Person(_)) => {
                    self.waiting = false;
                    self.heard = true;
                }
                Body::QuestionAnswered(_) if matches!(event.by, By::Person(_)) => self.heard = true,
                Body::TurnStarted(started) => {
                    let ours = started.trigger.is_some() && started.trigger == self.ours;
                    self.turn = ours.then(|| (TurnId::new(event.seq), false));
                    self.ours = None;
                }
                Body::MessageAssistant(reply) => {
                    if let Some((turn, called)) = self.turn.as_mut()
                        && event.turn == Some(*turn)
                        && reply
                            .blocks
                            .iter()
                            .any(|block| matches!(block, Block::ToolCall(_)))
                    {
                        *called = true;
                    }
                }
                Body::TurnEnded(_) => {
                    if let Some((_, called)) = self.turn.take() {
                        self.waiting |= !called;
                    }
                }
                _ => {}
            }
        }
    }

    /// 头上看到的样子（施工 X-7）：跑着、在等人开口的是等人。
    pub(super) fn shown(&self, mut goal: Goal) -> Goal {
        if goal.state == GoalState::Running && self.waiting {
            goal.state = GoalState::Waiting;
        }
        goal
    }

    /// 目标 `goal` 跑着：会话不闲够了退下（退下再载入就停了，要人接着跑）。
    pub(super) fn keeps(goal: Option<&Goal>) -> bool {
        goal.is_some_and(|goal| goal.state == GoalState::Running)
    }
}

/// `by` 是这个模块。
fn is_ours(by: &By) -> bool {
    matches!(by, By::Module(module) if module.id.as_str() == MODULE)
}

impl Actor {
    /// 送完一批以后：该开续轮的交回续轮消息，到了轮数上限的交回记卡住的命令；别的没有。
    pub(super) fn goal_round(&mut self) -> Option<Input> {
        let goal = self.counted().goals.now().cloned()?;
        let run = &self.goal_run;
        if goal.state != GoalState::Running || run.waiting || self.session.sealed() {
            return None;
        }
        if run.sent_at == Some(goal.round) || !self.session.vacant() || !self.inbox.is_empty() {
            return None;
        }
        self.goal_run.sent_at = Some(goal.round);
        let at = self.clock.now();
        let next = goal.round.saturating_add(1);
        // 编号照协议的约定写（施工 X-8 补）：头照它知道这是第几轮。
        let id = CommandId::parse(&round_command(next, at.unix_millis())).ok()?;
        let by = By::Module(Module {
            id: ModuleId::parse(MODULE).ok()?,
        });
        let command = match goal.limit {
            Some(limit) if goal.round >= limit.get() => Command::Goal(GoalChange::End {
                outcome: GoalOutcome::Blocked,
                reason: Some(format!("Reached the round limit ({limit}).")),
            }),
            _ => {
                let text = self.round_text(&goal, next)?;
                self.goal_run.full_sent = true;
                Command::Send {
                    blocks: vec![Block::Text(Text { text })],
                    urgent: false,
                    venue: None,
                }
            }
        };
        Some(Input::Command(Received {
            id,
            by,
            at,
            command,
        }))
    }

    /// 送完一批以后（施工 X-8 补）：头上看到的目标和上一次推的不一样，推一份（[`Pushed::Goal`]）。轮数、花的、等不等人都算。
    pub(super) fn push_goal(&mut self) {
        let goal = self
            .counted()
            .goals
            .now()
            .cloned()
            .map(|goal| self.goal_run.shown(goal));
        if goal == self.goal_run.pushed {
            return;
        }
        self.goal_run.pushed.clone_from(&goal);
        let at = self.clock.now();
        self.push(Pushed::Goal { at, goal });
    }

    /// 第 `round` 轮的续轮消息：完整版发过了的发短版；谁设的照目标填（施工 X-8 上）。读不了资源的记一行、不发。
    fn round_text(&self, goal: &Goal, round: u32) -> Option<String> {
        let resources = &self.persona.as_ref()?.resources;
        let texts = match resources.goal_rounds() {
            Ok(texts) => texts,
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "goal round text not read");
                return None;
            }
        };
        let template = if self.goal_run.full_sent {
            &texts.again
        } else {
            &texts.full
        };
        let set_by = if goal.by_you {
            &texts.by_you
        } else {
            &texts.by_person
        };
        let round = match goal.limit {
            Some(limit) => format!("Round {round} of {limit}"),
            None => format!("Round {round}"),
        };
        let objective = serde_json::to_string(&goal.text).ok()?;
        // 原文最后换：她写的原文里带着占位的样子也不再换。
        Some(
            template
                .replace("{round}", &round)
                .replace("{set_by}", set_by)
                .replace("{objective}", &objective),
        )
    }
}

mod asked;
#[cfg(test)]
mod tests;
