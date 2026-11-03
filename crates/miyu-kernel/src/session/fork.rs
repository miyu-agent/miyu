//! 分叉（施工 C-8 上，`docs/blueprint/session/fork.md`、`03-事件模型.md` 第七节）：子会话的历史引用父会话到分叉点为止的有效
//! 历史，不复制。底子只进有效历史：它的压缩、撤销、撤回照父会话自己的账本算；回合、任务、审批这些只照子会话自己的日志。
//! 子会话的日志接着分叉点往下数，第一条是带 `forked` 的 `session.created`。fork 子代理的分叉点在父会话这一轮当中（施工 A-F 上，
//! `within.rs`）。
//!
//! 底子一块块喂（施工 V-39 中，`feed.rs`）：分叉、载入都不再要整份底子在手上；分叉只写第一条，不攒底子的有效历史。

use super::Session;
#[cfg(any(test, feature = "testkit"))]
use super::action::Action;
use super::policy::Policy;
use crate::event::{Body, Event, Forked, Permission, SessionCreated};
use crate::facts::Environment;
use crate::history::History;
use crate::id::{CommandId, ContentHash, Seq, SessionId, TurnId};
use crate::ledger::{Ledger, LedgerError};
use crate::origin::By;
use crate::time::Timestamp;

use super::load::LoadError;

pub use feed::{BaseFeed, ForkFeed};
pub(in crate::session) use within::continued;

mod feed;
mod within;

/// 从父会话分叉要照的几样（施工 C-8 上）：分叉点，和那一刻父会话的权限、钉住的模型、策略快照。只照落了盘的日志算，一块块
/// 读着算（[`ForkFeed`]，施工 V-39 中）。
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
    /// 那一刻要用的人格（施工 P-13）：中途换过的照换的，分叉出来的照它记进 `session.created`。无人格的没有。
    pub persona: Option<String>,
    /// 那一刻要用的预设（施工 P-13）：同上；以前的日志没记的没有。
    pub preset: Option<String>,
    /// 分叉点在哪一轮当中（施工 A-F 上，[`ForkFeed::finish`] 给了派它的调用）：子会话的 `forked.within` 照它写；普通分叉没有。
    pub within: Option<TurnId>,
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
    /// 分叉出一个会话 `session`（施工 C-8 上），交回它的第一条（带 `forked` 的 `session.created`，序号接着分叉点）。`base` 是
    /// 喂到分叉点为止的父会话的底子（[`BaseFeed`]），`created` 写好了 `forked`（分叉点是 `base` 的最后一条）、`parent`、`depth`
    /// 和照父会话的那几格。只照底子的账本写第一条，不攒底子的有效历史（施工 V-39 中）：要接着用这个会话的，照底子和这几条
    /// 载入（[`Session::loader`]）。
    ///
    /// # Errors
    ///
    /// `created` 没写 `forked`、写了 `within`（那是 [`Session::fork_within`] 的）、底子过不了父会话的账本、停不在分叉点上或者
    /// 停在一轮当中：[`LoadError::Broken`]。
    #[allow(clippy::too_many_arguments, reason = "同 Session::create，多一样底子")]
    pub fn fork(
        session: SessionId,
        base: &BaseFeed,
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> Result<Vec<Event>, LoadError> {
        let opening = Opening {
            session,
            id,
            by,
            at,
        };
        let (_, events) = Session::fork_on(
            opening,
            base,
            History::whole(),
            None,
            created,
            policy,
            environment,
        )?;
        Ok(events)
    }

    /// fork 子代理（施工 A-F 上）：同 [`Session::fork`]，`created` 的 `forked.within` 写着分叉点在哪一轮当中，`base` 停在那一轮
    /// 派它的那一步上。头几条接着 `session.created` 记那一步每个调用的占位结果（字照 `placeholder`）、以 `forked` 收住那一轮。
    ///
    /// # Errors
    ///
    /// 同 [`Session::fork`]；没写 `within`、底子没停在那一轮里一步刚派出去的调用上：[`LoadError::Broken`]。
    #[allow(
        clippy::too_many_arguments,
        reason = "同 Session::fork，多一句占位结果"
    )]
    pub fn fork_within(
        session: SessionId,
        base: &BaseFeed,
        placeholder: &str,
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> Result<Vec<Event>, LoadError> {
        let opening = Opening {
            session,
            id,
            by,
            at,
        };
        let (_, events) = Session::fork_on(
            opening,
            base,
            History::whole(),
            Some(placeholder),
            created,
            policy,
            environment,
        )?;
        Ok(events)
    }

    /// 测试替身、测试里要接着驱动分叉出来的会话的：同 [`Session::fork`]，底子是父会话的日志里到分叉点为止的事件（整份），交回的
    /// 会话带着底子的有效历史（以前的 `Session::fork` 就是这样），动作里是那几条。
    ///
    /// # Errors
    ///
    /// 同 [`Session::fork`]。
    #[cfg(any(test, feature = "testkit"))]
    #[allow(clippy::too_many_arguments, reason = "同 Session::fork")]
    pub fn fork_live(
        session: SessionId,
        base: &[Event],
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> Result<(Session, Vec<Action>), LoadError> {
        Session::live(
            session,
            base,
            None,
            (id, by, at),
            created,
            policy,
            environment,
        )
    }

    /// 同 [`Session::fork_live`]，fork 子代理的（[`Session::fork_within`]）。
    ///
    /// # Errors
    ///
    /// 同 [`Session::fork_within`]。
    #[cfg(any(test, feature = "testkit"))]
    #[allow(clippy::too_many_arguments, reason = "同 Session::fork_within")]
    pub fn fork_within_live(
        session: SessionId,
        base: &[Event],
        placeholder: &str,
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> Result<(Session, Vec<Action>), LoadError> {
        Session::live(
            session,
            base,
            Some(placeholder),
            (id, by, at),
            created,
            policy,
            environment,
        )
    }

    /// [`Session::fork_live`]、[`Session::fork_within_live`] 共用的：底子整份当一块喂，有效历史照它从哪一条算起留着一切地收、
    /// 再落到检查点上。
    #[cfg(any(test, feature = "testkit"))]
    fn live(
        session: SessionId,
        base: &[Event],
        placeholder: Option<&str>,
        (id, by, at): (CommandId, By, Timestamp),
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> Result<(Session, Vec<Action>), LoadError> {
        let parent = created
            .forked
            .as_ref()
            .map_or_else(|| session.clone(), |forked| forked.from.clone());
        let feed = BaseFeed::of(&parent, base);
        let from = feed.ledger().compacted().map_or(Seq::FIRST, Seq::next);
        let mut history = History::whole();
        for event in base {
            match event.seq >= from {
                true => history.append(event.clone()),
                false => history.note(event),
            }
        }
        history.settle();
        let opening = Opening {
            session,
            id,
            by,
            at,
        };
        let (session, events) = Session::fork_on(
            opening,
            &feed,
            history,
            placeholder,
            created,
            policy,
            environment,
        )?;
        Ok((session, vec![Action::Append(events)]))
    }

    /// [`Session::fork`]、[`Session::fork_within`] 共用的：`placeholder` 有没有要和 `forked.within` 有没有对得上。`history` 是
    /// 交回的会话的底子的有效历史：只写第一条的是空的。
    fn fork_on(
        Opening {
            session,
            id,
            by,
            at,
        }: Opening,
        base: &BaseFeed,
        history: History,
        placeholder: Option<&str>,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> Result<(Session, Vec<Event>), LoadError> {
        let broken = |why: &str| {
            LoadError::Broken(LedgerError {
                seq: Seq::FIRST,
                why: why.to_string(),
            })
        };
        let forked = created
            .forked
            .clone()
            .ok_or_else(|| broken("a forked session.created names where it forks from"))?;
        let placeholder = match (forked.within, placeholder) {
            (Some(turn), Some(placeholder)) => Some((turn, placeholder)),
            (None, None) => None,
            (Some(_), None) => return Err(broken("a fork within a turn goes through fork_within")),
            (None, Some(_)) => return Err(broken("fork_within needs a fork point within a turn")),
        };
        base.check(&forked)?;
        let history = history
            .owned_by(session.clone())
            .forked_from(forked.from.clone());
        let first = base.last().map_or(Seq::FIRST, Seq::next);
        let ledger = continued(
            Ledger::for_session(session.clone())
                .starting_at(first)
                .heard_from(base.ledger()),
            forked.within,
            base.ledger(),
        );
        let mut session = Session::blank(session, ledger, history, &created, policy, environment);
        let mut events =
            vec![session.record(at, by, Some(id.clone()), Body::SessionCreated(created))];
        if let Some((turn, placeholder)) = placeholder {
            events.extend(session.settle_inherited(at, &id, turn, placeholder));
        }
        session.accept(id, events.iter().map(|event| event.seq).collect());
        Ok((session, events))
    }
}

/// 造分叉出来的会话的那一条命令：会话编号、命令编号、谁、什么时候。
struct Opening {
    session: SessionId,
    id: CommandId,
    by: By,
    at: Timestamp,
}
