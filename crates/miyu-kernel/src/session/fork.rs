//! 分叉（施工 C-8 上，`docs/blueprint/session/fork.md`、`03-事件模型.md` 第七节）：子会话的历史引用父会话到分叉点为止的有效
//! 历史，不复制。底子只进有效历史：它的压缩、撤销、撤回照父会话自己的账本算；回合、任务、审批这些只照子会话自己的日志。
//! 子会话的日志接着分叉点往下数，第一条是带 `forked` 的 `session.created`。

use super::Session;
use super::action::Action;
use super::configure::Reference;
use super::policy::Policy;
use crate::event::{Body, Event, Forked, Permission, PolicyChanged, SessionCreated};
use crate::facts::Environment;
use crate::history::History;
use crate::id::{CommandId, ContentHash, Seq, SessionId};
use crate::ledger::{Ledger, LedgerError};
use crate::origin::By;
use crate::time::Timestamp;

use super::load::LoadError;

/// 分叉出来的会话的底子：父会话的日志里到分叉点为止的事件，和它们的有效历史从哪一条算起。
pub(super) struct Base<'a> {
    /// 父会话。
    pub(super) parent: SessionId,
    /// 父会话那一段的有效历史从哪一条算起：它自己最近一次还算数的压缩替代到的下一条，没压缩过的是第 1 条。
    pub(super) from: Seq,
    /// 那一段事件，照原样。
    pub(super) events: &'a [Event],
    /// 那一段过完的父会话的账本：旁听记下的、当过触发的，子会话照它接着算。
    pub(super) ledger: Ledger,
}

impl<'a> Base<'a> {
    /// 照父会话的日志 `base`（从第 1 条起、照原样）和分叉的来源 `forked` 算底子：`base` 过一遍父会话自己的账本（认出哪几次
    /// 压缩还算数），要正好停在分叉点上，停的地方不能在一轮当中。
    pub(super) fn of(base: &'a [Event], forked: Forked) -> Result<Base<'a>, LoadError> {
        let broken = |seq: Seq, why: &str| {
            LoadError::Broken(LedgerError {
                seq,
                why: why.to_string(),
            })
        };
        let mut ledger = Ledger::for_session(forked.from.clone());
        for event in base {
            ledger.append(event).map_err(LoadError::Broken)?;
        }
        if base.last().map(|event| event.seq) != Some(forked.upto) {
            return Err(broken(
                forked.upto,
                "the parent log does not reach the fork point",
            ));
        }
        if ledger.open_turn().is_some() {
            return Err(broken(forked.upto, "the fork point is inside a turn"));
        }
        Ok(Base {
            parent: forked.from,
            from: ledger.compacted().map_or(Seq::FIRST, Seq::next),
            events: base,
            ledger,
        })
    }

    /// 底子的有效历史：照它从哪一条算起，留着一切地收、再落到检查点上。
    fn history(&self) -> History {
        let mut history = History::whole();
        for event in self.events {
            match event.seq >= self.from {
                true => history.append(event.clone()),
                false => history.note(event),
            }
        }
        history.settle();
        history
    }
}

/// 从父会话分叉要照的几样（施工 C-8 上）：分叉点，和那一刻父会话的权限、钉住的模型、策略快照。只照落了盘的日志算。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkPlan {
    /// 分叉点：父会话在跑一轮的，是这一轮 `turn.started` 之前那一条；闲着的是最后一条。
    pub upto: Seq,
    /// 那一刻的权限。
    pub permission: Permission,
    /// 那一刻钉住的模型的引用；没钉的没有。
    pub model: Option<String>,
    /// 那一刻手上的策略快照：最近换上的那一份，没换过的是造会话时的。
    pub policy: ContentHash,
}

/// 照父会话 `parent` 落了盘的日志 `events` 算分叉要照的几样（施工 C-8 上）：日志过一遍父会话的账本，停在一轮当中的，分叉点
/// 退到这一轮开始之前；权限、模型、快照照分叉点为止的。
///
/// # Errors
///
/// 日志过不了账本、是空的：[`LoadError`]。
pub fn fork_plan(parent: &SessionId, events: &[Event]) -> Result<ForkPlan, LoadError> {
    let mut ledger = Ledger::for_session(parent.clone());
    for event in events {
        ledger.append(event).map_err(LoadError::Broken)?;
    }
    let last = events.last().ok_or(LoadError::Empty)?.seq;
    let upto = match ledger.open_turn() {
        Some(turn) => Seq::new(turn.started().get() - 1).ok_or(LoadError::Empty)?,
        None => last,
    };
    let mut permission = None;
    let mut policy = None;
    let mut reference = Reference::default();
    for event in events.iter().take_while(|event| event.seq <= upto) {
        reference.note(event);
        match &event.body {
            Body::SessionCreated(created) => {
                permission = Some(created.permission.clone());
                policy = Some(created.policy.clone());
            }
            Body::PolicyChanged(PolicyChanged {
                permission: changed,
                policy: swapped,
                ..
            }) => {
                if let Some(changed) = changed {
                    permission = Some(changed.clone());
                }
                if let Some(swapped) = swapped {
                    policy = Some(swapped.clone());
                }
            }
            _ => {}
        }
    }
    let (Some(permission), Some(policy)) = (permission, policy) else {
        return Err(LoadError::Empty);
    };
    Ok(ForkPlan {
        upto,
        permission,
        model: reference.current().map(str::to_string),
        policy,
    })
}

/// 自己的日志 `own` 的第一条写的分叉来源；不是带 `forked` 的 `session.created` 的，日志坏了。
pub(super) fn forked_of(own: &[Event]) -> Result<Forked, LoadError> {
    let first = own.first();
    first
        .and_then(|event| match &event.body {
            Body::SessionCreated(created) => created.forked.clone(),
            _ => None,
        })
        .ok_or_else(|| {
            LoadError::Broken(LedgerError {
                seq: first.map_or(Seq::FIRST, |event| event.seq),
                why: "a forked session starts with a forked session.created".to_string(),
            })
        })
}

impl Session {
    /// 分叉出一个会话 `session`（施工 C-8 上）：`base` 是父会话的日志里到分叉点为止的事件，`created` 写好了 `forked`（分叉点
    /// 是 `base` 的最后一条）、`parent`、`depth` 和照父会话的那几格。交回的动作里是那一条 `session.created`，序号接着分叉点。
    ///
    /// # Errors
    ///
    /// `created` 没写 `forked`、底子过不了父会话的账本、停不在分叉点上或者停在一轮当中：[`LoadError::Broken`]。
    #[allow(clippy::too_many_arguments, reason = "同 Session::create，多一样底子")]
    pub fn fork(
        session: SessionId,
        base: &[Event],
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> Result<(Session, Vec<Action>), LoadError> {
        let forked = created.forked.clone().ok_or_else(|| {
            LoadError::Broken(LedgerError {
                seq: Seq::FIRST,
                why: "a forked session.created names where it forks from".to_string(),
            })
        })?;
        let base = Base::of(base, forked)?;
        let history = base
            .history()
            .owned_by(session.clone())
            .forked_from(base.parent.clone());
        let first = base
            .events
            .last()
            .map_or(Seq::FIRST, |event| event.seq.next());
        let ledger = Ledger::for_session(session.clone())
            .starting_at(first)
            .heard_from(&base.ledger);
        let mut session = Session::blank(session, ledger, history, &created, policy, environment);
        let event = session.record(at, by, Some(id.clone()), Body::SessionCreated(created));
        session.accept(id, vec![event.seq]);
        Ok((session, vec![Action::Append(vec![event])]))
    }
}
