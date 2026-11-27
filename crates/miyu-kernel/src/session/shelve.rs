//! 闲下来收成槽、要用时放回来（施工 LW-10 下，`internal/blueprint/kernel/session.md`「闲下来收成槽」）：会话歇着
//! （[`Session::resting`]）的时候执行器叫它收（[`Session::shelve`]），有效历史里的整条事件放掉、只记序号（`history/shelf.rs`）；
//! 下一条要看历史的输入送进来以前（[`Session::needs_history`]），执行器照 [`Session::shelved`] 从日志读回那一截交进来
//! （[`Session::unshelve`]），放回的和收起来以前一条不差。内核不读盘：读的是执行器。
//!
//! 群里旁听的话、`events.append` 记的、空闲时的「落了盘」不看历史，收着也直接送（施工 LW-36，[`Session::needs_history`]）：
//! 追加的先另放着，落了盘并进槽里只记序号（`history/shelf.rs`），所以一直有人说话的群照样收着，内核这一份不随条数长。
//!
//! 收着的时候还要答的几样查询：有没有能压的（[`Session::compactable`]）收的时候算好记下，收着时不再重算：限额、压缩、撤销这些
//! 会改变它的输入都要先放回来；旁听的话、`events.append` 记的不放回，会让它变旧，`compactable.changed` 可能晚到下一次放回以后
//! 才推（施工 LW-36 接受，真的 `session.compact` 先放回再判，结果不受影响）。派出去的任务、待办、包的状态本来就不收；这时的上下文
//! 用量（[`Session::context_used`]）收着时没有，执行器只在回合里要；上下文的明细（[`Session::breakdown`]）执行器先放回来再要。
//! 读不回来的那段时间（执行器照旧送了输入）投影看到的是空的历史，执行器一律不发请求（`session/actor.md`「闲下来收成槽」第 4 条）。

use super::Session;
use super::input::{Command, Input, Received};
use crate::event::Event;
use crate::history::Missing;
use crate::id::Seq;

impl Session {
    /// 歇着：空闲（[`Session::idle`]），追加的都落了盘，没有等落盘才回应的命令，回顾、起标题、提前压、替它看图都不在路上。执行器
    /// 照它判能不能收成槽：这时收了，下一条要看历史的输入多半是人下一句话。
    pub fn resting(&self) -> bool {
        self.idle()
            && self.unstored.is_empty()
            && self.waiting.is_empty()
            && self.recapping.is_none()
            && self.titling.is_none()
            && !self.prepare.asking()
            && !self.sight.looking()
    }

    /// 收成槽：歇着的才收，有效历史里的整条事件拿出来交回、只记序号；有没有能压的先算好记下。交回的由执行器放（在哪个线程放
    /// 由它定，`history/shelf.rs` 说了为什么）。不在歇着、已经收着的不收，交回没有。
    pub fn shelve(&mut self) -> Option<Vec<Event>> {
        if !self.resting() || self.shelved().is_some() {
            return None;
        }
        let compactable = self.compactable();
        let taken = self.history.shelve()?;
        self.rested = Some(compactable);
        Some(taken)
    }

    /// 收着的：要从日志的第几条读起，读到末尾，交给 [`Session::unshelve`]。分叉出来的会话可能早于自己的第一条：底子在父会话的
    /// 日志里，执行器连底子一起读。没收着的没有。
    pub fn shelved(&self) -> Option<Seq> {
        self.history.shelved(self.ledger.next_seq())
    }

    /// 放回来：`events` 是日志里从 [`Session::shelved`] 那一条起、照序号读到末尾的事件。放回的是在调的线程里拷的一份，`events`
    /// 照旧归调的一方放（`history/shelf.rs`）。没收着的什么都不做。
    ///
    /// # Errors
    ///
    /// 读回的里面少了要放回的那一条（日志被截短了、读的时候被改短了、那一行换了）：什么都不动，照旧收着。
    pub fn unshelve(&mut self, events: &[Event]) -> Result<(), Missing> {
        self.history.unshelve(events)?;
        self.rested = None;
        Ok(())
    }

    /// 收着的时候这一条要不要先放回来（`kernel/session.md`「闲下来收成槽」第 5 条）。不看历史、收着也直接送的：
    ///
    /// - 只改有没有头看着、在哪个目录的（`Watched`、`Environment`）；
    /// - 场所里旁听的一句（施工 LW-36）：带 `venue.ambient`、不是别处来的，和 `send_as` 认旁听的是同一个判法
    ///   （`send.rs` 的 `overheard`），只记下、不开回合；
    /// - `events.append` 记的一条（施工 LW-36）：不带回合编号、不开回合；
    /// - 空闲时（[`Session::idle`]）的「落了盘」（施工 LW-36）：推送、回应、并进槽，没有回合要往下走。回合里的、等着
    ///   `turn.ended` 落盘的要接着拼请求、跑挂接点，照旧先放回。
    ///
    /// 别的一律先放回来，`session.note` 也是：它记的事实空闲时排进下一轮开头，回合里跟着这一轮。
    pub fn needs_history(&self, input: &Input) -> bool {
        match input {
            Input::Watched { .. } | Input::Environment(_) => false,
            Input::Stored { .. } => !self.idle(),
            Input::Command(Received { by, command, .. }) => match command {
                Command::Send { venue, .. } => !self.overheard(by, venue.as_deref()),
                Command::Append { .. } => false,
                _ => true,
            },
            _ => true,
        }
    }

    /// 有效历史（替身对照用：收起来以前的和放回来的一样，`testkit/shelf.rs`）。
    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn history(&self) -> &crate::history::History {
        &self.history
    }

    /// 送进来一条要看历史的：收的时候记下的「有没有能压的」不算数了（读不回来照旧送的，这时投影看到的是空的历史）。
    pub(super) fn forget_rested(&mut self, input: &Input) {
        if self.needs_history(input) {
            self.rested = None;
        }
    }

    /// 收的时候记下的「有没有能压的」；没收着的没有。
    pub(super) fn rested_compactable(&self) -> Option<bool> {
        self.rested
    }
}
