//! 落盘和读回（施工 3-7 中；施工 6-9 从 `actor.rs` 挪出来，那边放不下了）：在阻塞线程里写一批，提交点的交给后台同步
//! （施工 V-19）；撤掉压缩时读回更早的日志（`docs/blueprint/session/actor.md` 第 4 条）。写不进去、读不回来，会话都停下：内存里的会话和磁盘对不上了，
//! 从磁盘重新载入最清楚（`07-存储.md` 第四节「写不进去」）。

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, CompactableChanged, Event, Transient, TransientBody};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::By;
use miyu_kernel::session::{Input, Outcome};

use std::io;
use std::sync::Arc;

use tokio::sync::oneshot;

use super::{Actor, Stop, answer};
use crate::TARGET;
use crate::current::{Current, Tally};
use crate::effects;
use crate::goal::Goals;
use crate::store::Flush;

impl Actor {
    /// 这一刻「当前的」几样（施工 9-6 上）：订阅时和订阅在同一步里交回。权限、还在跑的任务照内核这一刻的，累计的照给头看的
    /// 那一份。
    pub(super) fn current(&self) -> Current {
        // 只拿一次锁：同一条语句里拿两次，第一把要到语句结束才放，第二次就等死在它上面（施工 X-6 写测试时撞上的）。
        let counted = self.counted();
        Current {
            permission: self.session.permission().clone(),
            jobs: self.session.running_started(),
            nested: self.session.nested_running(),
            cwd: self.session.cwd().to_string(),
            dirs: self.session.dirs().to_vec(),
            tally: counted.tally.clone(),
            goal: counted
                .goals
                .now()
                .cloned()
                .map(|goal| self.goal_run.shown(goal)),
            persona: self.session.persona().map(str::to_string),
            preset: self.session.preset().map(str::to_string),
            compactable: self.session.compactable(),
        }
    }

    /// 送完一批以后推给头的（施工 X-8 补、W-12 补）：长期目标变了推一份；有没有能压的变了推一条 `compactable.changed`。头一回
    /// 算只记下：订阅的回应带着这一刻的。
    pub(super) fn push_now(&mut self) {
        self.push_goal();
        let now = self.session.compactable();
        let before = self.counted().compactable.replace(now);
        if before.is_none_or(|before| before == now) {
            return;
        }
        let (turn, cause) = self
            .session
            .turn_cause()
            .map_or((None, None), |(turn, cause)| (Some(turn), cause));
        let at = self.clock.now();
        self.push(crate::handle::Pushed::Transient(Transient {
            at,
            turn,
            by: By::Kernel,
            cause,
            body: TransientBody::CompactableChanged(CompactableChanged { compactable: now }),
        }));
    }

    /// 载入时照整份日志算的累计（施工 9-6 上）、长期目标（施工 X-6）：放进给头看的那一份，之后每落一批盘加上这一批。
    pub(crate) fn count_from(&self, tally: Tally, goals: Goals) {
        let mut counted = self.counted();
        counted.tally = tally;
        counted.goals = goals;
    }

    /// 给头看的那一份，拿着锁。
    pub(super) fn counted(&self) -> std::sync::MutexGuard<'_, crate::shown::Shown> {
        self.shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 在阻塞线程里写，写完交回「落盘了」；要同步的交给后台，不等它（施工 V-19，`durable.rs`）。写不进去就停下。撤销、
    /// 恢复落了盘，她看过的照日志重算一遍
    /// （施工 4-7 上）：重算不出来的记一条运行日志，照旧用原来的，改的工具照样先核对。
    pub(super) async fn append(&mut self, events: Vec<Event>) -> Result<Option<Input>, Stop> {
        let Some(upto) = events.last().map(|event| event.seq) else {
            return Ok(None);
        };
        let mut store = self.store.take().ok_or(Stop)?;
        let reseen = effects::reverts(&events);
        // 派出去的任务、回报记进名册（施工 7-4）：写不进去的，会话照样停下，记了也不要紧。
        self.jobs.note(&events);
        // 挂在刚报了开始的后台命令下面、等着的脚本调用，这时交进内核（施工 M-3，`tools/nested/port.rs`）。
        self.tools.note_jobs(&events);
        // 压缩、撤销、恢复了的，交过的子目录项目指令清掉重来（施工 P-14 下）。
        self.tools.note_docs(&events);
        // 人话带着她在场所里叫什么的，记忆记住最新的：她当场记的时候认她（施工 E-1 修三）。
        if let Some((memory, _)) = self.tools.memory() {
            memory.heard(&events);
        }
        // 累计的也是（施工 9-6 上）：写完、内核记下「落了盘」之前收件箱不动，订阅插不进来，和它拿到的落盘到哪一条对得上。
        {
            let mut counted = self.counted();
            counted.tally.add(&events);
            counted.goals.add(&events);
        }
        self.goal_run.note(&events);
        // 阻塞线程带着会话的 span：那边记的 `session index not updated` 也有会话编号（施工 3-8 七补）。
        let span = tracing::Span::current();
        let sync = must_sync(&events);
        // 同步挪到后台的（施工 V-19）：只写、不同步，写完拿最新的句柄；不挪的照旧连同同步写。
        let background = store.background();
        let written = tokio::task::spawn_blocking(move || {
            span.in_scope(|| {
                let result = match background {
                    true => store
                        .write(&events, false)
                        .and_then(|()| store.sync_handle().map(Some)),
                    false => store.write(&events, sync).map(|()| None),
                };
                let seen = (reseen && result.is_ok())
                    .then(|| store.events().map(|all| effects::seen_in(&all)));
                (store, result, seen)
            })
        })
        .await;
        match written {
            Ok((store, Ok(handle), seen)) => {
                self.store = Some(store);
                self.written(upto, sync, handle);
                match seen {
                    Some(Ok(seen)) => self.tools.see(seen),
                    Some(Err(error)) => {
                        tracing::warn!(target: TARGET, error = %error, "seen files not rebuilt");
                    }
                    None => {}
                }
                Ok(Some(Input::Stored {
                    at: self.clock.now(),
                    upto,
                }))
            }
            Ok((_, Err(error), _)) => {
                tracing::warn!(target: TARGET, kind = ?error.kind(), "write failed, stopped");
                Err(Stop)
            }
            Err(error) => {
                if error.is_panic() {
                    tracing::error!(target: TARGET, "panicked, stopped");
                }
                Err(Stop)
            }
        }
    }

    /// 读回日志（施工 6-9）：在阻塞线程里只读地一段一段读，交回第 `from` 条起的。读不了的（日志坏了、磁盘出错），记一行
    /// `read back failed, stopped`，会话停下；读的线程 panic 了，记一行 `panicked, stopped`。
    pub(super) async fn read_back(&mut self, from: Seq) -> Result<Input, Stop> {
        let store = self.store.take().ok_or(Stop)?;
        let read = tokio::task::spawn_blocking(move || {
            let events = store.events_from(from);
            (store, events)
        })
        .await;
        match read {
            Ok((store, Ok(events))) => {
                self.store = Some(store);
                Ok(Input::ReadBack {
                    at: self.clock.now(),
                    from,
                    events,
                })
            }
            Ok((_, Err(error))) => {
                tracing::warn!(target: TARGET, error = %error, "read back failed, stopped");
                Err(Stop)
            }
            Err(error) => {
                if error.is_panic() {
                    tracing::error!(target: TARGET, "panicked, stopped");
                }
                Err(Stop)
            }
        }
    }
}

impl Actor {
    /// 写进去了一批（施工 V-19，`session/actor.md` 第 5 条第 9 款）：有句柄的记下，要同步的交给后台；没有句柄的（不做后台
    /// 同步的）写的时候已经同步过，当场算同步到了这一条。
    fn written(&mut self, upto: Seq, sync: bool, handle: Option<Arc<dyn Flush>>) {
        match handle {
            Some(handle) => {
                self.sync.flush = Some(handle);
                if let Some(upto) = self.sync.durable.written(upto, sync) {
                    self.dispatch(upto);
                }
            }
            None => {
                self.sync.durable.written(upto, false);
                let (released, _) = self.sync.durable.done(upto);
                release(released);
            }
        }
    }

    /// 派一次后台同步，同步到 `upto`：在阻塞线程里对最新的句柄做，做完报回 actor（`synced`）。没有句柄的（还没写过、不做
    /// 后台同步的）当场算同步到了。
    pub(super) fn dispatch(&mut self, upto: Seq) {
        let Some(flush) = self.sync.flush.clone() else {
            let (released, _) = self.sync.durable.done(upto);
            return release(released);
        };
        let done = self.syncs.clone();
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let result = span.in_scope(|| flush.flush());
            // actor 已经退出了：没人收，丢掉。
            drop(done.send((upto, result)));
        });
    }

    /// 一次后台同步做完了：放出等到了的回应，该接着做的接着派。同步不了的和写不进去一样，记一行，会话停下。
    pub(super) fn synced(&mut self, upto: Seq, result: io::Result<()>) -> Result<(), Stop> {
        if let Err(error) = result {
            tracing::warn!(target: TARGET, kind = ?error.kind(), "sync failed, stopped");
            self.sync.failed = true;
            return Err(Stop);
        }
        let (released, next) = self.sync.durable.done(upto);
        release(released);
        if let Some(next) = next {
            self.dispatch(next);
        }
        Ok(())
    }

    /// 停下以前（第 11 款）：等路上的同步做完，把写了还没同步的同步完，交扣着的回应，放下句柄（Windows 上开着的句柄挡着挪
    /// 目录）。同步不了的记一行，扣着的回应不交：等的那一头收到会话停了。
    pub(super) async fn settle_sync(&mut self) {
        while self.sync.durable.busy() {
            let Some((upto, result)) = self.synced.recv().await else {
                break;
            };
            if self.synced(upto, result).is_err() {
                self.sync.flush = None;
                return;
            }
        }
        if let (Some(upto), Some(flush)) = (self.sync.durable.pending(), self.sync.flush.clone()) {
            match tokio::task::spawn_blocking(move || flush.flush()).await {
                Ok(Ok(())) => release(self.sync.durable.done(upto).0),
                Ok(Err(error)) => {
                    tracing::warn!(target: TARGET, kind = ?error.kind(), "sync failed, stopped");
                    self.sync.failed = true;
                    self.sync.flush = None;
                    return;
                }
                Err(_) => {
                    tracing::error!(target: TARGET, "panicked, stopped");
                    self.sync.flush = None;
                    return;
                }
            }
        }
        release(self.sync.durable.release_all());
        self.sync.flush = None;
    }
}

/// 交出放出来的回应。
fn release(released: Vec<(oneshot::Sender<Outcome>, Outcome)>) {
    for (reply, outcome) in released {
        answer(reply, outcome);
    }
}

/// 这一批要不要同步到磁盘（施工 V-18，`session/actor.md` 第 5 条第 1 款）。要的是「之后会碰外面」的几种：
///
/// - 人、别的会话、桥交来的（`by` 不是模型、工具、内核）：回应到了对方手里，断电以后就不能没了。
/// - 带工具调用的回复：落了盘才派工具，工具会改文件、跑命令；断电以后日志里要看得到它派过。
/// - 回合结束、会话造好、撤销和恢复：一轮收尾的地方。
///
/// 剩下的（模型的纯文字回复、工具的结果、`model.called`、注入的上下文……）只交给系统：程序崩了照样在，断电丢了的
/// 那一截载入时照「崩在回合里」收尾（`aborted`），和只差一两条的崩溃是同一条路。工具结果里引用的 blob 是先落盘的，
/// 丢了结果只是少了引用。
fn must_sync(events: &[Event]) -> bool {
    events.iter().any(|event| {
        let outside = !matches!(event.by, By::Model(_) | By::Tool(_) | By::Kernel);
        outside
            || match &event.body {
                Body::MessageAssistant(reply) => reply
                    .blocks
                    .iter()
                    .any(|block| matches!(block, Block::ToolCall(_))),
                Body::SessionCreated(_)
                | Body::TurnEnded(_)
                | Body::TurnReverted(_)
                | Body::TurnUnreverted(_) => true,
                _ => false,
            }
    })
}

#[cfg(test)]
mod tests;
