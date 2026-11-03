//! 一块块喂的底子（施工 V-39 中，`docs/construction/V-39中-支线底子边读边喂.md`）：父会话的日志照先后一块块交过来，过父会话
//! 的账本，有效历史的起点往前挪了，挪过去的那一截当场只记派出去的任务、待办（`History::note`），放掉（同施工 V-15 的
//! [`Loader`](super::super::Loader)）。载入支线（[`Session::loader`](super::super::Session::loader)）、分叉
//! （[`Session::fork`](super::super::Session::fork)）都吃它：以前两处都要整份底子在手上，父会话三万多条时一处五十多 MB。
//!
//! 分叉要先读到最后才知道分叉点在哪（父会话在跑一轮的，分叉点是这一轮开头的前一条），[`ForkFeed`] 让底子和权限、快照、引用
//! 这几样记账落后一轮：还没结束的那一轮先攒在一边，结束了再喂；读完照账本定分叉点，攒着的那一截只喂到分叉点。

use super::super::configure::Reference;
use super::super::load::LoadError;
use super::ForkPlan;
use crate::event::{Body, Event, Forked, Permission, PolicyChanged};
use crate::history::History;
use crate::id::{CallId, ContentHash, Seq, SessionId, TurnId};
use crate::ledger::{Ledger, LedgerError};

/// 一块块喂的底子（施工 V-39 中）：父会话的日志从第 1 条起照先后喂（[`BaseFeed::feed`]），喂到分叉点为止。过父会话自己的
/// 账本（认出哪几次压缩还算数）；有效历史只留还算数的最近一次压缩替代到的下一条起的，压缩往前挪了，挪过去的当场只记、
/// 放掉。过不了账本的先记着，交给它的时候（载入、分叉）照原样报，后面再喂的不看。
///
/// 起点往回挪了（父会话的压缩被撤了），放掉的就多了：交给载入以后 [`Loader::needs_whole`](super::super::Loader::needs_whole)
/// 是真，照整份重来（同施工 V-15）。分叉只看账本、最后一条，不看留下的。
#[derive(Debug)]
pub struct BaseFeed {
    /// 父会话：账本照它的编号记。
    parent: SessionId,
    /// 喂过的父会话的账本。
    ledger: Ledger,
    /// 第一条过不了账本的：记下以后不再喂。
    broken: Option<LedgerError>,
    /// 最后一条的序号，和它是不是 `message.assistant`（fork 子代理的底子要停在一步刚派出去的调用上）。
    last: Option<(Seq, bool)>,
    /// 有效历史：放掉的那一截只记了派出去的任务、待办。
    history: History,
    /// 还没放掉的，照先后：序号都不比 `noted` 小。
    kept: Vec<Event>,
    /// 序号比它小的都只记了、放掉了。
    noted: Seq,
}

/// 喂完、查过的底子：交给载入、分叉。
pub(in crate::session) struct Fed {
    /// 父会话。
    pub(in crate::session) parent: SessionId,
    /// 父会话那一段的有效历史从哪一条算起：它自己最近一次还算数的压缩替代到的下一条，没压缩过的是第 1 条。
    pub(in crate::session) from: Seq,
    /// 那一段过完的父会话的账本：旁听记下的、当过触发的，子会话照它接着算。
    pub(in crate::session) ledger: Ledger,
    /// 分叉点在哪一轮当中（施工 A-F 上）：fork 子代理接着那一轮；普通分叉没有。
    pub(in crate::session) within: Option<TurnId>,
    /// 放掉的那一截记过的有效历史。
    pub(in crate::session) history: History,
    /// 还没放掉的，照先后。
    pub(in crate::session) kept: Vec<Event>,
    /// 序号比它小的都放掉了。
    pub(in crate::session) noted: Seq,
}

impl BaseFeed {
    /// 从父会话 `parent` 的第 1 条起喂。
    #[must_use]
    pub fn new(parent: SessionId) -> BaseFeed {
        BaseFeed {
            ledger: Ledger::for_session(parent.clone()),
            parent,
            broken: None,
            last: None,
            history: History::whole(),
            kept: Vec::new(),
            noted: Seq::FIRST,
        }
    }

    /// 父会话 `parent` 的日志 `events`（从第 1 条起）整份当一块喂（拷一份）。
    #[must_use]
    pub fn of(parent: &SessionId, events: &[Event]) -> BaseFeed {
        let mut feed = BaseFeed::new(parent.clone());
        feed.feed(events.to_vec());
        feed
    }

    /// 喂一块，照先后：一条条过账本；喂完了起点往前挪了的，挪过去的那一截只记、放掉。
    pub fn feed(&mut self, events: Vec<Event>) {
        for event in events {
            self.push(event);
        }
        self.drop_replaced();
    }

    /// 喂一条，不放：过不了账本的记下，以后的都不要。
    pub(super) fn push(&mut self, event: Event) {
        if self.broken.is_some() {
            return;
        }
        if let Err(error) = self.ledger.append(&event) {
            self.broken = Some(error);
            self.kept = Vec::new();
            return;
        }
        self.last = Some((event.seq, matches!(event.body, Body::MessageAssistant(_))));
        self.kept.push(event);
    }

    /// 起点往前挪了的，挪过去的那一截只记派出去的任务、待办，放掉。
    pub(super) fn drop_replaced(&mut self) {
        let from = self.from();
        if from > self.noted {
            let gone = self.kept.partition_point(|event| event.seq < from);
            for event in self.kept.drain(..gone) {
                self.history.note(&event);
            }
            self.noted = from;
        }
    }

    /// 有效历史从第几条算起：还算数的最近一次压缩替代到的下一条，没有的是第 1 条。
    fn from(&self) -> Seq {
        self.ledger.compacted().map_or(Seq::FIRST, Seq::next)
    }

    /// 过不了账本的那一条。
    pub(in crate::session) fn broken(&self) -> Option<&LedgerError> {
        self.broken.as_ref()
    }

    /// 喂过的父会话的账本。
    pub(in crate::session) fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// 最后一条的序号：一条都没喂过的没有。
    pub(in crate::session) fn last(&self) -> Option<Seq> {
        self.last.map(|(seq, _)| seq)
    }

    /// 照分叉的来源 `forked` 查（同以前的 `Base::of`）：过得了父会话的账本，正好停在分叉点上。停的地方不能在一轮当中，fork
    /// 子代理的除外（`forked.within`，施工 A-F 上）：要停在它说的那一轮里、一步刚派出去的调用上（`within.rs`）。
    pub(in crate::session) fn check(&self, forked: &Forked) -> Result<(), LoadError> {
        let broken = |why: &str| {
            LoadError::Broken(LedgerError {
                seq: forked.upto,
                why: why.to_string(),
            })
        };
        if let Some(error) = &self.broken {
            return Err(LoadError::Broken(error.clone()));
        }
        if self.last() != Some(forked.upto) {
            return Err(broken("the parent log does not reach the fork point"));
        }
        let replied = self.last.is_some_and(|(_, replied)| replied);
        match forked.within {
            None if self.ledger.open_turn().is_some() => {
                Err(broken("the fork point is inside a turn"))
            }
            Some(turn) => super::within::check(replied, &self.ledger, turn).map_err(broken),
            None => Ok(()),
        }
    }

    /// 查过（[`BaseFeed::check`]），交出喂好的。
    pub(in crate::session) fn finish(self, forked: &Forked) -> Result<Fed, LoadError> {
        self.check(forked)?;
        let from = self.from();
        debug_assert_eq!(self.parent, forked.from, "底子照分叉的来源喂");
        Ok(Fed {
            parent: forked.from.clone(),
            from,
            ledger: self.ledger,
            within: forked.within,
            history: self.history,
            kept: self.kept,
            noted: self.noted,
        })
    }
}

/// 照父会话的日志一块块算分叉（施工 V-39 中）：分叉点、那一刻的权限、模型、快照、人格、预设（[`ForkPlan`]），和喂到分叉点
/// 为止的底子（[`BaseFeed`]）。同以前整份读进来过两遍账本再挑：账本过的、报的错一样，分叉点照读到的最后一条算。
///
/// 底子和那几样记账落后一轮：开着、还没结束的那一轮（从它的 `turn.started` 起）攒在一边，结束了再喂；读完照整份的账本定
/// 分叉点，攒着的那一截只喂到分叉点。喂进底子的每一条照先后交给调的一方的 `settled`（会话那一头照它记工作目录）：交的
/// 正好是到分叉点为止的每一条。
#[derive(Debug)]
pub struct ForkFeed {
    /// 喂到这一轮以前的底子。
    base: BaseFeed,
    /// 那几样记账，同样落后一轮。
    plan: Planning,
    /// 开着、还没结束的那一轮，从它的 `turn.started` 起，照先后。
    turn: Vec<Event>,
    /// 读到的最后一条。
    last: Option<Seq>,
}

impl ForkFeed {
    /// 从父会话 `parent` 的第 1 条起读。
    #[must_use]
    pub fn new(parent: SessionId) -> ForkFeed {
        ForkFeed {
            base: BaseFeed::new(parent),
            plan: Planning::default(),
            turn: Vec::new(),
            last: None,
        }
    }

    /// 喂一块，照先后。开着的那一轮攒着；别的喂进底子，一条条交给 `settled`。
    pub fn feed(&mut self, events: Vec<Event>, settled: &mut dyn FnMut(&Event)) {
        for event in events {
            self.last = Some(event.seq);
            if self.base.broken().is_some() {
                continue;
            }
            match &event.body {
                // 上一轮没结束又开一轮的，账本会拦下：照先后先喂上一轮，这一条接着攒，到时候照样报。
                Body::TurnStarted(_) => {
                    self.settle_turn(settled);
                    self.turn.push(event);
                }
                Body::TurnEnded(_) if !self.turn.is_empty() => {
                    self.turn.push(event);
                    self.settle_turn(settled);
                }
                _ if !self.turn.is_empty() => self.turn.push(event),
                _ => self.settle(event, settled),
            }
        }
        self.base.drop_replaced();
    }

    /// 读完了，定分叉点：`call` 是空的，同以前的 `fork_plan`（在跑一轮的，分叉点退到这一轮开始之前，闲着的是最后一条）；
    /// 不是空的，同以前的 `fork_plan_at`（fork 子代理，施工 A-F 上：分叉点是派 `call` 的那一步）。攒着的那一截喂到分叉点，
    /// 交回要照的几样和喂好的底子。
    ///
    /// # Errors
    ///
    /// 日志过不了账本、是空的：[`LoadError`]；`call` 不是正在进行的那一轮里等结果的调用：[`LoadError::Broken`]。
    pub fn finish(
        mut self,
        call: Option<CallId>,
        settled: &mut dyn FnMut(&Event),
    ) -> Result<(ForkPlan, BaseFeed), LoadError> {
        if let Some(error) = self.base.broken() {
            return Err(LoadError::Broken(error.clone()));
        }
        // 整份的账本：喂到这一轮以前的，接着过攒着的那一截。只在这里克隆一次，闲着的不用。
        let mut whole = None;
        if !self.turn.is_empty() {
            let mut ledger = self.base.ledger().clone();
            for event in &self.turn {
                ledger.append(event).map_err(LoadError::Broken)?;
            }
            whole = Some(ledger);
        }
        let ledger = whole.as_ref().unwrap_or(self.base.ledger());
        let last = self.last.ok_or(LoadError::Empty)?;
        let (upto, within) = match call {
            None => match ledger.open_turn() {
                Some(turn) => (
                    Seq::new(turn.started().get() - 1).ok_or(LoadError::Empty)?,
                    None,
                ),
                None => (last, None),
            },
            Some(call) => {
                let not_waiting = || {
                    LoadError::Broken(LedgerError {
                        seq: last,
                        why: format!(
                            "{call} is not a call waiting for a result in the running turn"
                        ),
                    })
                };
                let turn = ledger.open_turn().ok_or_else(not_waiting)?;
                if !ledger.pending_calls().contains(&call) {
                    return Err(not_waiting());
                }
                (ledger.last_reply().ok_or_else(not_waiting)?, Some(turn))
            }
        };
        drop(whole);
        for event in std::mem::take(&mut self.turn) {
            if event.seq > upto {
                break;
            }
            self.settle(event, settled);
        }
        self.base.drop_replaced();
        let plan = self.plan.finish(upto, within)?;
        Ok((plan, self.base))
    }

    /// 攒着的那一轮照先后喂进底子。
    fn settle_turn(&mut self, settled: &mut dyn FnMut(&Event)) {
        for event in std::mem::take(&mut self.turn) {
            self.settle(event, settled);
        }
    }

    /// 喂一条进底子、那几样记账，交给 `settled`。
    fn settle(&mut self, event: Event, settled: &mut dyn FnMut(&Event)) {
        self.plan.note(&event);
        settled(&event);
        self.base.push(event);
    }
}

/// 分叉那一刻的权限、快照、引用（同以前的 `plan_upto`）：照先后一条条记。
#[derive(Debug, Default)]
struct Planning {
    /// 权限：造会话时定的，被后来切过的盖掉。
    permission: Option<Permission>,
    /// 策略快照：造会话时的，被后来换上的盖掉。
    policy: Option<ContentHash>,
    /// 钉住的模型、人格、预设。
    reference: Reference,
}

impl Planning {
    /// 记一条。
    fn note(&mut self, event: &Event) {
        self.reference.note(event);
        match &event.body {
            Body::SessionCreated(created) => {
                self.permission = Some(created.permission.clone());
                self.policy = Some(created.policy.clone());
            }
            Body::PolicyChanged(PolicyChanged {
                permission: changed,
                policy: swapped,
                ..
            }) => {
                if let Some(changed) = changed {
                    self.permission = Some(changed.clone());
                }
                if let Some(swapped) = swapped {
                    self.policy = Some(swapped.clone());
                }
            }
            _ => {}
        }
    }

    /// 记到分叉点 `upto` 了：交回要照的几样，`within` 原样带上。
    fn finish(self, upto: Seq, within: Option<TurnId>) -> Result<ForkPlan, LoadError> {
        let (Some(permission), Some(policy)) = (self.permission, self.policy) else {
            return Err(LoadError::Empty);
        };
        Ok(ForkPlan {
            upto,
            permission,
            model: self.reference.current().map(str::to_string),
            policy,
            persona: self.reference.persona().map(str::to_string),
            preset: self.reference.preset().map(str::to_string),
            within,
        })
    }
}
