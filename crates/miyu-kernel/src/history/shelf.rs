//! 闲下来收成槽（施工 LW-10 下，`internal/blueprint/kernel/history.md`「闲下来收成槽」）：会话闲着、等人说话时，有效历史里的
//! 整条事件（检查点、之后还有效的、撤销放在一边的几截）都放掉，只记各是哪几条（序号，照原来的先后，连着的记成一段）。要用的
//! 时候执行器从日志读回这一截交进来（[`History::unshelve`]），照记下的序号原样放回：放回的和收起来以前一条不差，所以投影出的
//! 请求逐字节一样（日志里的每一行就是追加时的那一条，载入也靠这一点）。
//!
//! 放掉的整条交回执行器、放回的是在调的线程里重拷的一份，读来的那一份执行器自己放：glibc 的线程缓存（tcache）不分 arena，
//! 一个常驻的线程放掉别的线程（解日志的那几个）分配的小块，会留几块在自己的缓存里，钉住那几个 arena 的页、还不回系统。执行器
//! 把它们交给一个用完就退的线程去放，退的时候缓存还回各自的 arena（同 LiteMiyu 内核线第三趟乙的做法）。
//!
//! 派出去的任务、待办、包的状态、重读的原文、编号这几样小账照留：它们不随上下文涨（重读的原文不在日志里，读不回）。分叉出来的
//! 会话的底子在父会话的日志里：执行器读回时连底子一起读（`miyu-session` 的 `Store::events_from`），所以也收。
//!
//! 收着的时候又追加了的：任务、待办照常记，整条另放一边（`pending`），放回时先照槽放回，再把它们照先后接上，和一直没收一样。
//! 群里旁听的话、`events.append` 记的收着也直接送（施工 LW-36，`session/shelve.rs` 的 `needs_history`），一直有人说话的群
//! 另放的会一直长：所以落了盘的、只是接在后面的那几种（`history.rs` 的 `follows`）从另放的最前面起并进槽里，只记序号
//! （[`History::landed`]）。要等落盘：放回时从日志读它们，没落盘的日志里还没有，读回就少一条。压缩、撤销、恢复、撤回、
//! `turn.started` 不并，它们的输入本来就先放回（读不回、照旧送了的才会留在这里）；夹着它们的，它们和后面的都照旧另放，先后不乱。
//! 读不回的这段时间投影看到的是空的历史，执行器不发请求（`session/actor.md`「闲下来收成槽」）。

use crate::event::Event;
use crate::id::Seq;

use super::{History, follows, forget_texts};

/// 收起来的有效历史：各是哪几条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Shelf {
    /// 检查点是第几条；没压缩过的没有。
    checkpoint: Option<Seq>,
    /// 检查点之后还有效的，照原来的先后。
    events: Vec<Run>,
    /// 撤销放在一边的几截，最近的一次在最后。
    undone: Vec<Vec<Run>>,
    /// 收着的时候又追加的，照先后（见模块的说明）。
    pending: Vec<Event>,
}

/// 连着的一段：从第 `from` 条起 `len` 条。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    from: u64,
    len: u64,
}

/// 读回的日志里少了要放回的那一条：日志被截短了、读的时候被改短了、那一行被换掉了，或者交进来的那一截不是从要的地方起。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// 少的是第几条。
    pub seq: Seq,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the log has no event {} to put back", self.seq)
    }
}

impl std::error::Error for Missing {}

impl Shelf {
    /// 要从第几条读起：记着的序号里最小的；一条都没记的没有。
    fn from(&self) -> Option<Seq> {
        let runs = self.events.iter().chain(self.undone.iter().flatten());
        runs.map(|run| run.from)
            .chain(self.checkpoint.map(Seq::get))
            .min()
            .and_then(Seq::new)
    }
}

impl History {
    /// 收成槽（见模块的说明）：整条事件拿出来交回（执行器去放），只记序号。留着一切的那一份、已经收着的不收，交回没有。
    pub(crate) fn shelve(&mut self) -> Option<Vec<Event>> {
        if self.whole || self.shelf.is_some() {
            return None;
        }
        let mut taken = std::mem::take(&mut self.events);
        let undone = std::mem::take(&mut self.undone);
        self.shelf = Some(Shelf {
            checkpoint: self.checkpoint.as_ref().map(|event| event.seq),
            events: runs(&taken),
            undone: undone.iter().map(|run| runs(run)).collect(),
            pending: Vec::new(),
        });
        taken.extend(self.checkpoint.take());
        taken.extend(undone.into_iter().flatten());
        Some(taken)
    }

    /// 收着的：要从第几条读起（[`History::unshelve`] 要第几条起到日志末尾的）。没收着的没有；收着、一条都没记的交 `next`
    /// （下一条该是几号，读不出东西）。
    pub(crate) fn shelved(&self, next: Seq) -> Option<Seq> {
        self.shelf
            .as_ref()
            .map(|shelf| shelf.from().unwrap_or(next))
    }

    /// 收着的时候追加的一条（见模块的说明）：已经记过任务、待办，去掉了改了的文件的原文。没收着的照原样交回。
    pub(super) fn hold(&mut self, event: Event) -> Option<Event> {
        match &mut self.shelf {
            Some(shelf) => {
                shelf.pending.push(event);
                None
            }
            None => Some(event),
        }
    }

    /// 落到了第 `upto` 条（施工 LW-36，见模块的说明）：收着时另放的那一份，从最前面起、落了盘的、只是接在后面的，并进槽里只记
    /// 序号，碰到没落盘的、不是接在后面的那一种就停。没收着的什么都不做。
    pub(crate) fn landed(&mut self, upto: Seq) {
        let Some(shelf) = &mut self.shelf else {
            return;
        };
        let ready = shelf
            .pending
            .iter()
            .take_while(|event| event.seq <= upto && follows(&event.body))
            .count();
        for event in shelf.pending.drain(..ready) {
            let seq = event.seq.get();
            match shelf.events.last_mut() {
                Some(run) if run.from + run.len == seq => run.len += 1,
                _ => shelf.events.push(Run { from: seq, len: 1 }),
            }
        }
    }

    /// 放回来：`read` 是日志里从 [`History::shelved`] 那一条起、照序号排着的事件。照记下的序号原样放回（在调的线程里拷一份，
    /// `read` 照旧是调的一方的，见模块的说明），改了的文件的原文去掉（同追加，内核不读），再把收着时追加的照先后接上。没收着
    /// 的什么都不做。
    ///
    /// # Errors
    ///
    /// `read` 里少了要放回的那一条：什么都不动，照旧收着。
    pub(crate) fn unshelve(&mut self, read: &[Event]) -> Result<(), Missing> {
        let Some(shelf) = &self.shelf else {
            return Ok(());
        };
        let take = |seq: u64| -> Result<Event, Missing> {
            let missing = || Missing {
                seq: Seq::new(seq).unwrap_or(Seq::FIRST),
            };
            let at = read
                .binary_search_by_key(&seq, |event| event.seq.get())
                .map_err(|_| missing())?;
            let mut event = read[at].clone();
            forget_texts(&mut event);
            Ok(event)
        };
        let checkpoint = shelf.checkpoint.map(|seq| take(seq.get())).transpose()?;
        let events = put_back(&shelf.events, &take)?;
        let undone = shelf
            .undone
            .iter()
            .map(|runs| put_back(runs, &take))
            .collect::<Result<Vec<_>, _>>()?;
        let pending = self
            .shelf
            .take()
            .map(|shelf| shelf.pending)
            .unwrap_or_default();
        self.checkpoint = checkpoint;
        self.events = events;
        self.undone = undone;
        for event in pending {
            self.apply(event);
        }
        Ok(())
    }
}

/// 一串事件的序号，照先后，连着的记成一段。
fn runs(events: &[Event]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for event in events {
        let seq = event.seq.get();
        match runs.last_mut() {
            Some(run) if run.from + run.len == seq => run.len += 1,
            _ => runs.push(Run { from: seq, len: 1 }),
        }
    }
    runs.shrink_to_fit();
    runs
}

/// 照几段序号一条条取回来，照先后。
fn put_back(
    runs: &[Run],
    take: &impl Fn(u64) -> Result<Event, Missing>,
) -> Result<Vec<Event>, Missing> {
    let total = runs.iter().map(|run| run.len).sum::<u64>();
    let mut events = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
    for run in runs {
        for seq in run.from..run.from + run.len {
            events.push(take(seq)?);
        }
    }
    Ok(events)
}
