//! 一块块载入（施工 V-15，`docs/construction/V-15-重开大会话边读边重放.md`）：读日志的一方解好一块交一块，账本、几样记账
//! 照先后一条条过；有效历史只留还算数的最近一次压缩替代到的下一条起的，压缩往前挪了，挪过去的那一截当场只记派出去的任务、
//! 待办（`History::note`），放掉：整份不同时留在内存里。
//!
//! 和一次收完再过两遍的一样（施工 6-9，`docs/blueprint/kernel/history.md`「载入」）：先放掉的都在最后算出的起点前面。两种
//! 情况一块块喂对不上，[`Loader::needs_whole`] 是真，调的一方照整份日志走 [`Session::load`]：压缩被撤了（撤掉的回合里的压缩
//! 不算数），最后算出的起点比已经放掉的还早；会话的策略是读完整份才认得出的（最近一次换的快照），向上回报的截法和开头
//! 照着过账本的那一份不一样。

use super::{LoadError, Replay};
use crate::event::{Body, Event};
use crate::history::History;
use crate::id::{Seq, SessionId};
use crate::ledger::Ledger;
use crate::session::Session;
use crate::session::configure::Reference;
use crate::session::fork::{Base, forked_of};
use crate::session::grants::Grants;
use crate::session::jobs;
use crate::session::policy::{Policy, Reports};
use crate::session::report::Duty;
use crate::session::seal::Rounds;
use crate::session::sight::Sight;
use crate::session::title::Naming;

/// 载入到一半的会话：一块块喂（[`Loader::feed`]），喂完了收尾（`Loader::finish`，`load.rs`）。
pub struct Loader {
    /// 哪个会话。
    pub(in crate::session) session: SessionId,
    /// 向上回报的截法：过账本时照它算。
    reports: Reports,
    /// 分叉出来的：父会话、父会话那一段有效历史的起点。
    pub(in crate::session) parent: Option<(SessionId, Seq)>,
    pub(in crate::session) ledger: Ledger,
    pub(in crate::session) replay: Replay,
    pub(in crate::session) duty: Duty,
    pub(in crate::session) naming: Naming,
    pub(in crate::session) reference: Reference,
    pub(in crate::session) sight: Sight,
    pub(in crate::session) grants: Grants,
    pub(in crate::session) rounds: Rounds,
    /// 有效历史：放掉的那一截只记了派出去的任务、待办。
    pub(in crate::session) history: History,
    /// 还没定进不进有效历史的，照先后：序号都不比 `noted` 小。
    pub(in crate::session) kept: Vec<Event>,
    /// 序号比它小的都只记了、放掉了。
    noted: Seq,
}

impl Session {
    /// 开始一块块载入会话 `session`（施工 V-15）：`first` 是日志的头一块（没有事件的是空的），`base` 是分叉出来的会话的底子
    /// （同 [`Session::load_forked`]），`reports` 是这时认得出的策略里向上回报的截法。头一块不在这里喂，接着交给
    /// [`Loader::feed`]。
    ///
    /// # Errors
    ///
    /// 分叉出来的：头一条不是带 `forked` 的 `session.created`，底子过不了父会话的账本、和头一条接不上。
    pub fn loader(
        session: SessionId,
        base: Option<Vec<Event>>,
        first: &[Event],
        reports: Reports,
    ) -> Result<Loader, LoadError> {
        let start = first.first().map_or(Seq::FIRST, |event| event.seq);
        let mut ledger = Ledger::for_session(session.clone()).starting_at(start);
        let (parent, base) = match base {
            Some(base) => {
                let of = Base::of(&base, forked_of(first)?)?;
                ledger = ledger.heard_from(&of.ledger);
                (Some((of.parent, of.from)), base)
            }
            None => (None, Vec::new()),
        };
        Ok(Loader {
            session,
            reports,
            parent,
            kept: base,
            ledger,
            replay: Replay::default(),
            duty: Duty::default(),
            naming: Naming::default(),
            reference: Reference::default(),
            sight: Sight::default(),
            grants: Grants::default(),
            rounds: Rounds::default(),
            history: History::whole(),
            noted: Seq::FIRST,
        })
    }
}

impl Loader {
    /// 喂一块：照先后一条条过账本、记账；有效历史的起点往前挪了的，挪过去的那一截只记、放掉。
    ///
    /// # Errors
    ///
    /// 有一条过不了账本。
    pub fn feed(&mut self, events: Vec<Event>) -> Result<(), LoadError> {
        for event in events {
            // 向上回报照活着时的算（施工 7-6）：活着时落了盘就报，接着开的一轮和结束在同一批里，那时不该报。
            if !matches!(event.body, Body::TurnStarted(_)) && self.duty.due(&self.ledger) {
                self.duty.take(&self.reports);
            }
            let queued = self.ledger.queued();
            self.ledger.append(&event).map_err(LoadError::Broken)?;
            self.duty.note(&event);
            self.naming.note(&event);
            self.reference.note(&event);
            self.sight.note(&event);
            self.grants.note(&event);
            self.rounds.note(&event);
            let waking = jobs::waking(&self.ledger, &event);
            self.replay.note(&event, queued, waking);
            self.kept.push(event);
        }
        let from = self.from();
        if from > self.noted {
            let gone = self.kept.partition_point(|event| event.seq < from);
            for event in self.kept.drain(..gone) {
                self.history.note(&event);
            }
            self.noted = from;
        }
        Ok(())
    }

    /// 有效历史从第几条算起：还算数的最近一次压缩替代到的下一条；没有的，分叉出来的照父会话那一段的起点，别的是第 1 条。
    pub(in crate::session) fn from(&self) -> Seq {
        self.ledger
            .compacted()
            .map(Seq::next)
            .or_else(|| self.parent.as_ref().map(|(_, from)| *from))
            .unwrap_or(Seq::FIRST)
    }

    /// 一块块喂的对不上，要照整份日志走 [`Session::load`]：起点比已经放掉的还早（压缩被撤了），或者会话的策略 `policy`
    /// 向上回报的截法和开头照着过账本的不一样。
    #[must_use]
    pub fn needs_whole(&self, policy: &Policy) -> bool {
        self.from() < self.noted || policy.reports != self.reports
    }
}
