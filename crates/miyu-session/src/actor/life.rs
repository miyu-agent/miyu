//! actor 的一生（施工 V-2 再补从 `actor.rs` 挪来，那个文件到了行数上限）：起任务、看着它；闲够了退下（`docs/designs/07-存储.md`
//! 第七节「会话按需载入」第 3 条，`docs/blueprint/session/actor.md`「退下」）。
//!
//! 退不退由 actor 自己照账答：最后一次有动静以后过了多久，内核空不空（没有回合、没在重启、没有在等的子代理），还有没有头看着、
//! 等着回应的命令、在路上的请求、派出去还没回报或者结束了还没落盘的任务、记忆的闹钟、它在等的和等它空下来的会话，收件箱、
//! 回报的路里还有没有没办的。都没有、闲够了的答「退下了」就退出；回答交不出去的（会话表不等了）不退。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;
use tracing::Instrument;

use miyu_kernel::id::SessionId;
use miyu_kernel::session::Action;

use super::Actor;
use super::mail::Mail;
use crate::TARGET;
use crate::handle::{Leaving, Retire};
use crate::shown::Shown;

/// 会话的 span：开在 `ERROR` 级。span 也照级别筛，开在 `INFO` 的话，调到 `WARN` 它就被筛掉了，底下的
/// 行就没了会话编号（`miyu-log` 的说明，施工 3-7 上）。
pub(crate) fn span(id: &SessionId) -> tracing::Span {
    tracing::error_span!(target: TARGET, "session", session = id.as_str())
}

/// 起一个 actor 的任务，外面再套一个看着它的：它 panic 了（内核自己的 bug、端口的 bug），记一条
/// `ERROR`，别的会话照常（`28-运行日志.md` 第三节：`ERROR` 一定是 bug）。
pub(crate) fn spawn(actor: Actor, first: Vec<Action>, span: tracing::Span) {
    let busy = actor.busy();
    let task = tokio::spawn(actor.run(first).instrument(span.clone()));
    tokio::spawn(
        async move {
            if let Err(error) = task.await
                && error.is_panic()
            {
                tracing::error!(target: TARGET, "panicked, stopped");
            }
            // 停了的会话不算在跑：核心不为它不肯空闲退出。
            busy.store(false, Ordering::Release);
            // 会话放下的还给系统（23 F3，施工 V-2 再补）：大会话关掉以后 glibc 不还，核心的内存一直不回落。
            drop(tokio::task::spawn_blocking(miyu_heap::trim));
        }
        .instrument(span),
    );
}

impl Actor {
    /// 有没有在跑的回合：交给 `Handle` 的那一份（施工 V-51 从 `actor.rs` 挪来，那个文件到了行数上限）。
    pub(crate) fn busy(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.busy)
    }

    /// 给头看的限额和模型：交给 `Handle` 的那一份（施工 8-9、8-10）。
    pub(crate) fn shown(&self) -> Arc<Mutex<Shown>> {
        Arc::clone(&self.shown)
    }

    /// 这时有没有至少一个头订阅着：交给 `Handle` 的那一份（施工 C-5）。
    pub(crate) fn watched(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.watched)
    }

    /// 封存了的退得下了的那面旗：交给 `Handle` 的那一份（施工 V-51）。
    pub(crate) fn leaving(&self) -> Leaving {
        Leaving::of(&self.bell)
    }

    /// 每办完一封看一次（施工 V-51）：封存了的、照 [`Actor::occupied`] 没事了，立起旗告诉会话表来问，不等它隔一阵来问（最多
    /// 3 分钟）；又有事了放下。来不来问、问了退不退照旧：会话表看有没有回合、有没有人订阅、有没有别的把手，actor 照这同一本账答。
    /// 没封存的不看：闲够了才退，照会话表隔一阵问。
    pub(super) fn offer_retire(&self) {
        if self.session.sealed() {
            self.bell.offer(!self.occupied());
        }
    }

    /// 会话表问闲够 `idle` 了没有：照自己的账答，答了「退下了」的交回 [`Mail::Retired`]，actor 退出。封存了的（施工 C-8 下）
    /// 没事了就退，不等闲够：不会再开轮。问过就放下旗（施工 V-51）：没退的，下一次看时照账再立起来。
    pub(super) fn retire(&mut self, idle: Duration, reply: oneshot::Sender<Retire>) -> Mail {
        self.bell.lower();
        let answer = match self.occupied() {
            true => Retire::Kept,
            false if self.session.sealed() => Retire::Retired,
            false => match idle.checked_sub(self.quiet.elapsed()) {
                Some(left) if !left.is_zero() => Retire::Later(left),
                _ => Retire::Retired,
            },
        };
        match (answer, reply.send(answer)) {
            (Retire::Retired, Ok(())) => {
                tracing::info!(target: TARGET, "retired");
                Mail::Retired
            }
            _ => Mail::Done,
        }
    }

    /// 还有事：退下了会丢掉、或者要等它办完的。
    fn occupied(&self) -> bool {
        !self.session.vacant()
            || self.watchers > 0
            || !self.replies.is_empty()
            || !self.calls.is_empty()
            || !self.asides.is_empty()
            || !self.jobs.settled()
            || self.extracting()
            || !self.waiters.is_empty()
            || !self.watches.is_empty()
            || !self.inbox.is_empty()
            || !self.back.is_empty()
            || super::goal::GoalRun::keeps(self.counted().goals.now())
    }
}
