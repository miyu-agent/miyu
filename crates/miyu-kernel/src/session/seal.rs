//! 跑完几轮封存（施工 C-8 下，`docs/blueprint/session/fork.md`「封存」）：分叉时写了 `seal_after` 的，跑满这几轮、这一轮结束、没有
//! 接着开的，同一批在 `turn.ended` 后面记一条 `session.sealed`。之后不再开轮：改对话的命令拒 `session_sealed`，别处来的回报、
//! 留言照记、不叫醒她（`jobs.rs` 的「这时开得了一轮」）。
//!
//! 数轮照日志数，载入和活着时一个算法：有触发的一轮开了算一轮；接着开的不另算，它的 `trigger` 落在上一轮的开头和结尾之间
//! （上一轮里排着队的消息、并进来的 `turn.joined`、中途到的回报；被有计划的重启打断以后接着干的，触发是那一条结束），
//! `triggers` 是空的（照记下的几条开的是新的一轮）。手动压缩、清空单开的那一轮没有触发，不算。

use std::num::NonZeroU32;

use super::Session;
use super::input::Command;
use crate::event::{Body, Event, SessionSealed};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

/// 跑了几轮：从日志一条条算。
#[derive(Debug, Default)]
pub(super) struct Rounds {
    /// 跑满几轮封存：`session.created` 的 `forked` 里写的，没写的不封存。
    after: Option<NonZeroU32>,
    /// 开过几轮，接着开的不另算。
    counted: u32,
    /// 正在跑的那一轮从第几条开始。
    started: Option<Seq>,
    /// 最近结束的那一轮的开头和结尾。
    last: Option<(Seq, Seq)>,
}

impl Rounds {
    /// 记下追加的一条。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::SessionCreated(created) => {
                self.after = created.forked.as_ref().and_then(|forked| forked.seal_after);
            }
            Body::TurnStarted(started) => {
                let goes_on = started.triggers.is_empty()
                    && started
                        .trigger
                        .zip(self.last)
                        .is_some_and(|(trigger, (from, to))| from < trigger && trigger <= to);
                if started.trigger.is_some() && !goes_on {
                    self.counted = self.counted.saturating_add(1);
                }
                self.started = Some(event.seq);
            }
            Body::TurnEnded(_) => self.last = self.started.take().map(|from| (from, event.seq)),
            _ => {}
        }
    }

    /// 跑满了。
    fn done(&self) -> bool {
        self.after.is_some_and(|after| self.counted >= after.get())
    }
}

impl Session {
    /// 封存了没有（施工 C-8 下）：会话 actor 照它在空闲时当场退下，不等闲够。
    pub fn sealed(&self) -> bool {
        self.ledger.sealed()
    }

    /// 一轮结束了、没有接着开的：跑满了轮数、还没封存的，记一条 `session.sealed`，`by` 内核，`cause` 同那一条 `turn.ended`。
    pub(super) fn seal_if_done(
        &mut self,
        at: Timestamp,
        cause: Option<CommandId>,
    ) -> Option<Event> {
        if self.ledger.sealed() || !self.rounds.done() {
            return None;
        }
        let body = Body::SessionSealed(SessionSealed {});
        Some(self.record(at, By::Kernel, cause, body))
    }
}

/// 封存了还收不收这个命令：改对话的不收；改标题置顶、打断、回答、回顾，和别处来的回报、空了的通知照收（回报、通知只记下，
/// 不叫醒她）。一个个列出来，加一种命令时编译器会逼着决定。
pub(super) fn taken_when_sealed(command: &Command) -> bool {
    match command {
        Command::SetMeta { .. }
        | Command::Interrupt { .. }
        | Command::Answer { .. }
        | Command::Recap
        | Command::Report(_)
        | Command::PeerIdle { .. } => true,
        Command::Send { .. }
        | Command::Typed { .. }
        | Command::Respond { .. }
        | Command::Note { .. }
        | Command::Append { .. }
        | Command::SetPermission { .. }
        | Command::Configure { .. }
        | Command::Recast { .. }
        | Command::SetWorkspace { .. }
        | Command::Ran { .. }
        | Command::Revert { .. }
        | Command::Unrevert
        | Command::Redo { .. }
        | Command::Compact { .. }
        | Command::Clear
        | Command::ClearTodos
        | Command::Goal(_) => false,
    }
}
