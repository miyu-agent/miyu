//! 写盘的端口（施工 3-7 中「我定的」）：actor 追加的事件往哪里写。平时是会话日志；测试里换成写不进去
//! 的，查停下的那条路。不对外。撤销、恢复以后重算她看过的，从这里重读一遍日志（施工 4-7 上）；撤掉压缩时读回更早的
//! 一段，也从这里读（施工 6-9）。
//!
//! 交给 `history` 的日志只读入口也在这里（施工 6-4）：照会话的目录一段一段读。
//!
//! 平时写的是 [`Indexed`]：会话日志，每落一批顺手更新会话列表的索引里这个会话的那一行（施工 3-8 七补），写进用量汇总
//! （施工 8-15），更新回合索引（施工 R-2 上）。

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId};
use miyu_store::index::SessionIndex;
use miyu_store::log::{SessionLog, read_events, read_segments};
use miyu_store::usage::{UsageIndex, Who};
use miyu_tool::ReadLog;

use crate::TARGET;
use crate::memory::{Calls, Source, TurnLane};

/// 一次写一批，返回时这一批都落了盘。在阻塞线程里用，所以要能挪到别的线程上。
pub(crate) trait Store: Send + 'static {
    /// 追加一批。
    fn append(&mut self, events: &[Event]) -> io::Result<()>;

    /// 只读地读回整份日志。
    fn events(&self) -> Result<Vec<Event>, String>;

    /// 只读地读回第 `from` 条起的日志（施工 6-9）：一段一段读，只留要的。
    fn events_from(&self, from: Seq) -> Result<Vec<Event>, String>;

    /// 日志在哪个目录（施工 R-6 上）：抽取在别的线程里只读地读它，不占 actor。不在磁盘上的（测试里）没有。
    fn dir(&self) -> Option<PathBuf> {
        None
    }

    /// 换了人格，记忆挪了一间（施工 P-13）：照底子、自己的日志重接回合索引，交回新的记忆的端口。没有回合索引的照整份
    /// 日志接，接上的回合索引不留。
    fn rewire(&mut self, connect: Rewire) -> Result<Option<Calls>, String> {
        let source = Source {
            dir: self.dir().unwrap_or_default(),
            base: None,
        };
        Ok(connect(source, self.events()?).1)
    }
}

/// 照会话的日志（底子在哪、日志在哪）、读回的会话自己的整份事件接上记忆（施工 P-13，[`Store::rewire`]）。
pub(crate) type Rewire =
    Box<dyn FnOnce(Source, Vec<Event>) -> (Option<TurnLane>, Option<Calls>) + Send>;

impl Store for SessionLog {
    fn append(&mut self, events: &[Event]) -> io::Result<()> {
        SessionLog::append(self, events)
    }

    fn events(&self) -> Result<Vec<Event>, String> {
        read_events(self.dir()).map_err(|error| error.to_string())
    }

    fn events_from(&self, from: Seq) -> Result<Vec<Event>, String> {
        let mut kept = Vec::new();
        read_segments(self.dir(), |segment| {
            kept.extend(segment.into_iter().filter(|event| event.seq >= from));
            true
        })
        .map_err(|error| error.to_string())?;
        Ok(kept)
    }
    fn dir(&self) -> Option<PathBuf> {
        Some(SessionLog::dir(self).to_path_buf())
    }
}

/// 会话日志，和会话列表的索引里这个会话的那一行（施工 3-8 七补，`session/actor.md` 第 5 条第 7 点）、用量汇总里这一批
/// 发出去了的请求（施工 8-15）。
pub(crate) struct Indexed {
    log: SessionLog,
    /// 会话编号。
    id: SessionId,
    /// 索引：没有的不更新。
    index: Option<Arc<SessionIndex>>,
    /// 用量汇总，和这个会话的属主、场所、父会话：没有的不写。
    usage: Option<(Arc<UsageIndex>, Who)>,
    /// 回合索引（施工 R-2 上）：主会话、核心交了登记的才有。在后台写（施工 V-9）。
    turns: Option<TurnLane>,
    /// 分叉出来的会话的底子（施工 C-8 上）：父会话的日志里到分叉点为止的事件。读回日志时接在自己的前面，撤销读回到分叉点
    /// 以前的也接得上。
    base: Option<Arc<Vec<Event>>>,
}

impl Indexed {
    /// 日志 `log` 落了盘的每一批，照会话 `id` 更新 `index` 里的那一行，写进用量汇总 `usage`（属主等照 `who`），更新回合
    /// 索引 `turns`。
    pub(crate) fn new(
        log: SessionLog,
        id: &SessionId,
        index: Option<Arc<SessionIndex>>,
        usage: Option<(Arc<UsageIndex>, Who)>,
        turns: Option<TurnLane>,
    ) -> Indexed {
        Indexed {
            log,
            id: id.clone(),
            index,
            usage,
            turns,
            base: None,
        }
    }

    /// 同一个，读回日志时先接上底子 `base`（施工 C-8 上）。
    #[must_use]
    pub(crate) fn on_base(mut self, base: Arc<Vec<Event>>) -> Indexed {
        self.base = Some(base);
        self
    }

    /// 底子里第 `from` 条起的。
    fn base_from(&self, from: Seq) -> Vec<Event> {
        self.base
            .iter()
            .flat_map(|base| base.iter())
            .filter(|event| event.seq >= from)
            .cloned()
            .collect()
    }
}

impl Store for Indexed {
    /// 先落盘，再更新索引、写用量汇总、更新回合索引。更新失败只记一行 `session index not updated`、`usage not indexed`、
    /// `memory index not updated`，照样算写成了：三样都是派生的，停在上一次照到的地方，下次照日志补上。
    fn append(&mut self, events: &[Event]) -> io::Result<()> {
        let before = self.log.mark();
        self.log.append(events)?;
        if events.is_empty() {
            return Ok(());
        }
        let after = self.log.mark();
        if let Some(index) = &self.index
            && let Err(error) = index.advance(&self.id, &before, events, &after)
        {
            tracing::warn!(target: TARGET, error = %error, "session index not updated");
        }
        if let Some((usage, who)) = &self.usage
            && let Err(error) = usage.advance(&self.id, who, &before, events, &after)
        {
            tracing::warn!(target: TARGET, error = %error, "usage not indexed");
        }
        if let Some(turns) = &mut self.turns {
            turns.advance(events);
        }
        Ok(())
    }

    fn events(&self) -> Result<Vec<Event>, String> {
        let mut events = self.base_from(Seq::FIRST);
        events.extend(self.log.events()?);
        Ok(events)
    }

    fn events_from(&self, from: Seq) -> Result<Vec<Event>, String> {
        let mut events = self.base_from(from);
        events.extend(self.log.events_from(from)?);
        Ok(events)
    }

    fn dir(&self) -> Option<PathBuf> {
        Store::dir(&self.log)
    }

    /// 读不了日志的回合索引一并放掉：不往原来那一间接着记。
    fn rewire(&mut self, connect: Rewire) -> Result<Option<Calls>, String> {
        self.turns = None;
        let own = self.log.events()?;
        let source = Source::of(&self.log, self.base.clone());
        let (mut turns, calls) = connect(source, own);
        if let Some(turns) = &mut turns {
            turns.start();
        }
        self.turns = turns;
        Ok(calls)
    }
}

/// 这个会话日志的只读入口（施工 6-4）：会话的目录。
pub(crate) struct LogDir(pub(crate) PathBuf);

impl ReadLog for LogDir {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        read_segments(&self.0, each).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests;
