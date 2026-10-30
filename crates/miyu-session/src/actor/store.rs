//! 落盘和读回（施工 3-7 中；施工 6-9 从 `actor.rs` 挪出来，那边放不下了）：在阻塞线程里写一批，提交点的交给后台同步
//! （施工 V-19）；撤掉压缩时读回更早的日志（`docs/blueprint/session/actor.md` 第 4 条）。写不进去、读不回来，会话都停下：内存里的会话和磁盘对不上了，
//! 从磁盘重新载入最清楚（`07-存储.md` 第四节「写不进去」）。

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, CompactableChanged, Event, Transient, TransientBody};
use miyu_kernel::id::{CommandId, Seq};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Input, Outcome};

use std::io;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use tokio::sync::oneshot;

use super::Synced;
use super::durable::Syncing;
use super::{Actor, Stop, answer, durable};
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
            model: self.session.reference().map(str::to_string),
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
                let mark = store.mark();
                (store, result, seen, mark)
            })
        })
        .await;
        match written {
            Ok((store, Ok(handle), seen, mark)) => {
                self.store = Some(store);
                // 抽取记一处读起的位置（施工 E-5 补）：这一批里有回合结束的。
                if let (Some((memory, _)), Some(mark)) = (self.tools.memory(), mark) {
                    memory.landed(mark);
                }
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
            Ok((_, Err(error), _, _)) => {
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
        let mut shared = lock(&self.sync);
        match handle {
            Some(handle) => {
                shared.flush = Some(handle);
                let start = shared.durable.written(upto, sync);
                drop(shared);
                if let Some(upto) = start {
                    self.dispatch(upto);
                }
            }
            None => {
                shared.durable.written(upto, false);
                let (released, _) = shared.durable.done(upto);
                drop(shared);
                release(released);
            }
        }
    }

    /// 回应命令 `id`：交给等着它的最早的那一头。
    pub(super) fn reply(&mut self, id: CommandId, outcome: Outcome) {
        let Some(waiting) = self.replies.get_mut(&id) else {
            return;
        };
        let first = waiting.pop_front();
        if waiting.is_empty() {
            self.replies.remove(&id);
        }
        let Some((reply, hold)) = first else {
            return;
        };
        // 外面来的、接受了、带着事件的，事件都同步了才交（施工 V-19，`session/actor.md` 第 5 条第 10 款）；别的当场交。
        if !hold {
            return answer(reply, outcome);
        }
        let start = {
            let mut shared = self
                .sync
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            durable::route(&mut shared.durable, reply, outcome)
        };
        if let Some(upto) = start {
            self.dispatch(upto);
        }
    }

    /// 派一次后台同步，同步到 `upto`（`dispatch`）。
    pub(super) fn dispatch(&self, upto: Seq) {
        dispatch(Arc::clone(&self.sync), self.syncs.clone(), upto);
    }

    /// 一串后台同步做完了报回来的：出错的和写不进去一样，记一行，会话停下（扣着的回应不再交，`Syncing` 记着出过错）。
    pub(super) fn synced(&mut self, result: io::Result<()>) -> Result<(), Stop> {
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                tracing::warn!(target: TARGET, kind = ?error.kind(), "sync failed, stopped");
                Err(Stop)
            }
        }
    }

    /// 停下以前（第 11 款）：等路上的同步做完，把写了还没同步的同步完，交扣着的回应，放下句柄（Windows 上开着的句柄挡着挪
    /// 目录）。同步不了的记一行，扣着的回应不交：等的那一头收到会话停了。
    pub(super) async fn settle_sync(&mut self) {
        while lock(&self.sync).durable.busy() {
            match self.synced.recv().await {
                Some((_, Ok(()))) => {}
                Some((_, Err(error))) => {
                    tracing::warn!(target: TARGET, kind = ?error.kind(), "sync failed, stopped");
                    return;
                }
                None => break,
            }
        }
        let (pending, flush) = {
            let shared = lock(&self.sync);
            (shared.durable.pending(), shared.flush.clone())
        };
        if let (Some(upto), Some(flush)) = (pending, flush) {
            match tokio::task::spawn_blocking(move || flush.flush()).await {
                Ok(Ok(())) => {
                    let released = lock(&self.sync).durable.done(upto).0;
                    release(released);
                }
                Ok(Err(error)) => {
                    tracing::warn!(target: TARGET, kind = ?error.kind(), "sync failed, stopped");
                    lock(&self.sync).failed = true;
                    return;
                }
                Err(_) => {
                    tracing::error!(target: TARGET, "panicked, stopped");
                    lock(&self.sync).failed = true;
                    return;
                }
            }
        }
        let mut shared = lock(&self.sync);
        let rest = shared.durable.release_all();
        shared.flush = None;
        drop(shared);
        release(rest);
    }
}

/// 派一串后台同步，从同步到 `upto` 起（施工 V-19）：在阻塞线程里对最新的句柄做，做完当场放出等到了的回应，账上还要再做的
/// 接着做，都做完了、或者出了错，报回 actor（`synced`）。放回应不经过 actor：它正等着别的（权限策略在判）时，回应照样一落盘
/// 就交出去。
fn dispatch(shared: Arc<Mutex<Syncing>>, done: mpsc::UnboundedSender<Synced>, mut upto: Seq) {
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        loop {
            let flush = lock(&shared).flush.clone();
            let result = flush.map_or(Ok(()), |flush| flush.flush());
            let mut syncing = lock(&shared);
            if let Err(error) = result {
                syncing.failed = true;
                drop(syncing);
                // actor 已经退出了：没人收，丢掉。
                drop(done.send((upto, Err(error))));
                return;
            }
            let (released, next) = syncing.durable.done(upto);
            drop(syncing);
            release(released);
            match next {
                Some(next) => upto = next,
                None => {
                    drop(done.send((upto, Ok(()))));
                    return;
                }
            }
        }
    });
}

/// 拿着账的锁：拿锁的那一方 panic 过的照样用（账上的都是完整的一步一步记的）。
fn lock(shared: &Mutex<Syncing>) -> std::sync::MutexGuard<'_, Syncing> {
    shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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
