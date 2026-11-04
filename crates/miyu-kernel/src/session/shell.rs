//! 人在头的输入框里用 `!` 打的命令（施工 SH-1 上，设计 57 第三节，`docs/blueprint/kernel/session.md`「人跑的命令」）：
//! `message.user` 写了 `shell` 的那一句，核心在她听到它的那一刻跑，不是收到时跑。
//!
//! 听到的那一刻是这一轮的「准备好」（`turn.rs` 的 `advance`）：开轮的挂接点跑完了、两步之间问模型以前、回合结束接着开的
//! 那一轮第一件。还没跑的照先后一条一条跑：发 `RunShell`、这一轮停在 [`Stage::Shell`] 等执行器交回 `ShellRan`，记下、落了盘
//! 回到「准备好」，再跑下一条或者问模型。排着时退回的（`message.withdrawn`）不跑。
//!
//! 打断时在跑的：叫执行器停（`HaltShell`），等它交回到这时的输出，最多等 [`super::interrupt::STOP_WAIT_MS`]；还没跑、也不
//! 接着发的那几条记 `interrupted`、不跑（`close_interrupted`）。有计划的重启时在跑的记 `aborted`；崩了的、重启以后不接着干
//! 的，载入时补 `aborted`（`load.rs` 的 `recover`）。都不重跑。

use std::collections::VecDeque;

use super::Session;
use super::action::Action;
use super::turn::{Interrupting, Stage};
use crate::event::{Body, Event, ShellRan, ShellReason};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

/// 还没跑的几条，照序号的先后（提交时、载入时都喂）：写了 `shell` 的 `message.user` 进来，它的 `shell.ran`、撤回它的
/// `message.withdrawn` 拿走。
#[derive(Debug, Clone, Default)]
pub(super) struct Shells(VecDeque<Unrun>);

/// 还没跑的一条：哪一句、命令行、那句话的 `cause`。
#[derive(Debug, Clone)]
struct Unrun {
    message: Seq,
    command: String,
    cause: Option<CommandId>,
}

impl Shells {
    /// 看一条事件。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::MessageUser(message) if message.shell => self.0.push_back(Unrun {
                message: event.seq,
                command: message.text(),
                cause: event.cause.clone(),
            }),
            Body::ShellRan(ran) => self.0.retain(|unrun| unrun.message != ran.message),
            Body::MessageWithdrawn(withdrawn) => self
                .0
                .retain(|unrun| !withdrawn.messages.contains(&unrun.message)),
            _ => {}
        }
    }
}

impl Session {
    /// 「准备好」的这一刻有还没跑的：最早的那一条交给执行器跑，这一轮等它。没有的交回 `None`。
    pub(super) fn run_shell(&mut self) -> Option<Vec<Action>> {
        let unrun = self.shells.0.front()?.clone();
        let turn = self.turn.as_mut()?;
        turn.stage = Stage::Shell {
            message: unrun.message,
        };
        Some(vec![Action::RunShell {
            message: unrun.message,
            command: unrun.command,
            cwd: self.environment.cwd.clone(),
        }])
    }

    /// 执行器交回了跑完的一条：记 `shell.ran`（`by` 是内核，`cause` 照那句话的），回到「准备好」，落了盘接着走。打断以后在
    /// 等它的，记下就收尾。对不上在跑的那一条的（打断时不等了、迟到的）不理。
    pub(super) fn shell_ran(&mut self, at: Timestamp, ran: ShellRan) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        if !matches!(turn.stage, Stage::Shell { message } if message == ran.message) {
            return Vec::new();
        }
        let waiting = turn.interrupting.take();
        turn.stage = match waiting {
            Some(_) => Stage::Settling,
            None => Stage::Ready,
        };
        let cause = self.shell_cause(ran.message);
        let mut events = vec![self.record(at, By::Kernel, cause, Body::ShellRan(ran))];
        if let Some(Interrupting {
            by, cause, queued, ..
        }) = waiting
        {
            events.extend(self.close_interrupted(at, &by, &cause, queued));
        }
        vec![Action::Append(events)]
    }

    /// 打断时在跑的那一条（`interrupt.rs`）：叫执行器停，先不收尾；回应当场交，到点叫醒，记号是那一句的序号。
    pub(super) fn halt_shell(
        &mut self,
        at: Timestamp,
        id: CommandId,
        waiting: Interrupting,
        message: Seq,
    ) -> Vec<Action> {
        let wake = waiting.wake;
        if let Some(turn) = self.turn.as_mut() {
            turn.stage = Stage::Shell { message };
            turn.interrupting = Some(waiting);
        }
        self.recent.insert(id.clone(), Vec::new());
        vec![
            super::accepted(id, Vec::new()),
            Action::HaltShell { message },
            Action::Wake {
                at: super::interrupt::later(at, super::interrupt::STOP_WAIT_MS),
                seen: wake,
            },
        ]
    }

    /// 还没跑完、`gone` 认下的那几条不会再跑了，各补一条 `shell.ran`，`reason` 是 `reason`，没有输出。`by`、`cause` 照交的。
    pub(super) fn give_up_shells(
        &mut self,
        at: Timestamp,
        (by, cause): (&By, Option<CommandId>),
        reason: ShellReason,
        gone: impl Fn(Seq) -> bool,
    ) -> Vec<Event> {
        let gone: Vec<Seq> = self
            .shells
            .0
            .iter()
            .map(|unrun| unrun.message)
            .filter(|&message| gone(message))
            .collect();
        gone.into_iter()
            .map(|message| {
                let ran = ShellRan {
                    message,
                    exit: None,
                    signal: None,
                    duration_ms: 0,
                    text: String::new(),
                    omitted: 0,
                    reason: Some(reason.clone()),
                };
                self.record(at, by.clone(), cause.clone(), Body::ShellRan(ran))
            })
            .collect()
    }

    /// 那句话的 `cause`：还没跑的里面找。
    fn shell_cause(&self, message: Seq) -> Option<CommandId> {
        self.shells
            .0
            .iter()
            .find(|unrun| unrun.message == message)
            .and_then(|unrun| unrun.cause.clone())
    }
}
