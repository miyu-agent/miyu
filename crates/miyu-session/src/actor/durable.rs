//! 后台同步和扣着的回应（施工 V-19，`docs/blueprint/session/actor.md` 第 5 条第 9 到 11 款）：写进去就往下走，提交点的
//! 那几批交给后台同步，回应命令等它的事件都同步了才交出去。这里只管账：写到哪、同步到哪、正在做的同步到哪、还要做到
//! 哪、扣着哪些回应；不碰盘，碰盘的在 `actor/store.rs`。
//!
//! 同步一次只做一个：做的时候又来了要同步的，记下最远的那一条，做完接着派一次，几批合成一次（盘忙时一次同步要几秒，
//! 攒在一起的就不用各等各的）。

use std::sync::Arc;

use tokio::sync::oneshot;

use miyu_kernel::id::Seq;
use miyu_kernel::origin::By;
use miyu_kernel::session::Outcome;

use super::answer;
use crate::store::Flush;

/// 后台同步的账，`T` 是扣着的回应。
#[derive(Debug)]
pub(crate) struct Durable<T> {
    /// 写进去的最后一条。
    written: Option<Seq>,
    /// 同步到了哪一条（含）。
    synced: Option<Seq>,
    /// 正在做的同步到哪一条。
    flying: Option<Seq>,
    /// 正在做的时候又要同步的，最远到哪一条：做完接着派。
    wanted: Option<Seq>,
    /// 扣着的回应：要等哪一条同步了，照先后。
    held: Vec<(Seq, T)>,
}

impl<T> Default for Durable<T> {
    fn default() -> Durable<T> {
        Durable {
            written: None,
            synced: None,
            flying: None,
            wanted: None,
            held: Vec::new(),
        }
    }
}

/// 一个回应交不交得出去。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Held<T> {
    /// 它要的那一条已经同步了：当场交。
    Now(T),
    /// 扣着；带着的是该当场派的同步（同步到哪），没有的是已经有同步在路上、会盖到它。
    Waiting(Option<Seq>),
}

impl<T> Durable<T> {
    /// 写进去了一批，最后一条是 `upto`；`must` 的这一批要同步（V-18 的提交点）。交回该当场派的同步：同步到哪。
    pub(crate) fn written(&mut self, upto: Seq, must: bool) -> Option<Seq> {
        self.written = Some(upto);
        match must {
            true => self.ask(upto),
            false => None,
        }
    }

    /// 一个要等 `need` 同步了才交的回应。`need` 已经同步了的当场交；没有的扣着，没有同步在路上的补派一次，同步到写进去的
    /// 最后一条。
    pub(crate) fn hold(&mut self, need: Seq, item: T) -> Held<T> {
        if self.synced.is_some_and(|synced| synced >= need) {
            return Held::Now(item);
        }
        self.held.push((need, item));
        // 没有同步在路上的，派一次，同步到写进去的最后一条；在路上的只记下要到 `need`：路上那一次盖到了的，做完不会再派。
        let upto = match self.flying {
            None => self.written.map_or(need, |written| written.max(need)),
            Some(_) => need,
        };
        Held::Waiting(self.ask(upto))
    }

    /// 一次同步做完了，同步到 `upto`：交回放出来的回应（照先后），和该接着派的同步。
    pub(crate) fn done(&mut self, upto: Seq) -> (Vec<T>, Option<Seq>) {
        self.synced = self.synced.max(Some(upto));
        self.flying = None;
        let synced = self.synced;
        let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|(need, _)| synced.is_some_and(|synced| synced >= *need));
        self.held = waiting;
        let next = self.wanted.take().filter(|wanted| Some(*wanted) > synced);
        self.flying = next;
        (ready.into_iter().map(|(_, item)| item).collect(), next)
    }

    /// 有没有同步在路上（停下以前要等它做完：它拿着日志的句柄，Windows 上挡着挪目录）。
    pub(crate) fn busy(&self) -> bool {
        self.flying.is_some()
    }

    /// 写了还没同步的最后一条：停下以前要同步到这里。都同步了的没有。
    pub(crate) fn pending(&self) -> Option<Seq> {
        self.written.filter(|written| Some(*written) > self.synced)
    }

    /// 停下时把扣着的回应一次放完（先同步完了再放），照先后。
    pub(crate) fn release_all(&mut self) -> Vec<T> {
        std::mem::take(&mut self.held)
            .into_iter()
            .map(|(_, item)| item)
            .collect()
    }

    /// 要同步到 `upto`：没有在做的交回它（派出去），在做的记下最远的那一条。
    fn ask(&mut self, upto: Seq) -> Option<Seq> {
        match self.flying {
            None => {
                self.flying = Some(upto);
                Some(upto)
            }
            Some(_) => {
                self.wanted = self.wanted.max(Some(upto));
                None
            }
        }
    }
}

/// actor 拿着的那一份（施工 V-19）：账、最新的句柄、同步出没出过错。actor 不论怎么退出（停下、写不进去、panic 展开），
/// 扣着的回应都在 `Drop` 里收尾：先尽力同步一次，成了照接受交出去（事件已经写进去了，载入以后它在）；同步出过错的不交，
/// 等的那一头收到会话停了（`session/actor.md` 第 5 条第 11 款）。
pub(crate) struct Syncing {
    /// 后台同步的账和扣着的回应。
    pub(crate) durable: Durable<(oneshot::Sender<Outcome>, Outcome)>,
    /// 后台同步用的句柄：每写一批换成最新的（`Store::sync_handle`）；不做后台同步的没有。
    pub(crate) flush: Option<Arc<dyn Flush>>,
    /// 同步出过错：扣着的回应不再交。
    pub(crate) failed: bool,
}

impl Syncing {
    /// 还没写过的。
    pub(crate) fn new() -> Syncing {
        Syncing {
            durable: Durable::default(),
            flush: None,
            failed: false,
        }
    }
}

impl Drop for Syncing {
    fn drop(&mut self) {
        if self.failed {
            return;
        }
        let held = self.durable.release_all();
        if held.is_empty() {
            return;
        }
        let synced = match (self.durable.pending(), &self.flush) {
            (Some(_), Some(flush)) => flush.flush().is_ok(),
            _ => true,
        };
        if synced {
            for (reply, outcome) in held {
                answer(reply, outcome);
            }
        }
    }
}

/// 交一个回应（第 10 款）：不用等的、要的那一条已经同步了的当场交给 `reply`；要等的扣着。交回该派的同步（同步到哪）。
pub(crate) fn route(
    durable: &mut Durable<(oneshot::Sender<Outcome>, Outcome)>,
    reply: oneshot::Sender<Outcome>,
    outcome: Outcome,
) -> Option<Seq> {
    let Some(need) = waits_for(&outcome) else {
        answer(reply, outcome);
        return None;
    };
    match durable.hold(need, (reply, outcome)) {
        Held::Now((reply, outcome)) => {
            answer(reply, outcome);
            None
        }
        Held::Waiting(upto) => upto,
    }
}

/// 发命令的是外面的（人、桥、别的 harness），回应才扣着等同步（第 10 款）：交到它们手里的是断电也在的承诺。别的会话、定时、内核、
/// 模块、工具发的当场交：两头都在这个核心里、同一块盘上，扣着保不住什么，只会打乱会话之间的先后（带话又订的那一边要等
/// 回应才记下在等，对方先忙完了就再也等不到）。
pub(crate) fn outside(by: &By) -> bool {
    match by {
        By::Person(_) | By::External(_) | By::Harness(_) | By::Unknown(_) => true,
        By::Model(_)
        | By::Tool(_)
        | By::Module(_)
        | By::Session(_)
        | By::Schedule(_)
        | By::Kernel => false,
    }
}

/// 一个回应要等哪一条同步了才交（第 10 款）：接受了、带着事件的，等最后那一条；拒绝的、没有事件的、回顾的当场交。
pub(crate) fn waits_for(outcome: &Outcome) -> Option<Seq> {
    match outcome {
        Outcome::Accepted { events } => events.iter().max().copied(),
        Outcome::Recapped { .. } | Outcome::Rejected { .. } => None,
    }
}

#[cfg(test)]
mod tests;
