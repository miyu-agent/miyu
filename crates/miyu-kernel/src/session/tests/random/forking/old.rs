//! 施工 V-39 中以前的分叉、载入支线，原样抄来当准绳（origin/main `b8f496958` 的 `session/fork.rs`、`fork/within.rs`、
//! `load/loader.rs`、`load.rs`）：整份底子在手上，两遍过账本，分叉时把底子的有效历史攒一遍。只改了名字（方法改成函数）。

use crate::event::{Body, Event, Forked, PolicyChanged, SessionCreated};
use crate::facts::Environment;
use crate::history::History;
use crate::id::{CallId, CommandId, Seq, SessionId, TurnId};
use crate::ledger::{Ledger, LedgerError};
use crate::origin::By;
use crate::session::Session;
use crate::session::action::Action;
use crate::session::configure::Reference;
use crate::session::fork::ForkPlan;
use crate::session::grants::Grants;
use crate::session::load::{LoadError, Loader, Replay};
use crate::session::policy::{Policy, Reports};
use crate::session::report::Duty;
use crate::session::seal::Rounds;
use crate::session::sight::Sight;
use crate::session::title::Naming;
use crate::time::Timestamp;

/// 分叉出来的会话的底子：父会话的日志里到分叉点为止的事件，和它们的有效历史从哪一条算起。
pub(super) struct Base<'a> {
    pub(super) parent: SessionId,
    pub(super) from: Seq,
    pub(super) events: &'a [Event],
    pub(super) ledger: Ledger,
    pub(super) within: Option<TurnId>,
}

impl<'a> Base<'a> {
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
        match forked.within {
            None if ledger.open_turn().is_some() => {
                return Err(broken(forked.upto, "the fork point is inside a turn"));
            }
            Some(turn) => check(base, &ledger, turn).map_err(|why| broken(forked.upto, why))?,
            None => {}
        }
        Ok(Base {
            parent: forked.from,
            from: ledger.compacted().map_or(Seq::FIRST, Seq::next),
            events: base,
            ledger,
            within: forked.within,
        })
    }

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

fn check(base: &[Event], ledger: &Ledger, turn: TurnId) -> Result<(), &'static str> {
    if ledger.open_turn() != Some(turn) {
        return Err("a fork within a turn stops inside that turn");
    }
    match base.last().map(|event| &event.body) {
        Some(Body::MessageAssistant(_)) if !ledger.pending_calls().is_empty() => Ok(()),
        _ => Err("a fork within a turn stops right after a step that calls tools"),
    }
}

fn continued(ledger: Ledger, base: &Base<'_>) -> Ledger {
    match base.within {
        Some(_) => ledger.continuing(&base.ledger),
        None => ledger,
    }
}

pub(super) fn fork_plan(parent: &SessionId, events: &[Event]) -> Result<ForkPlan, LoadError> {
    let mut ledger = Ledger::for_session(parent.clone());
    for event in events {
        ledger.append(event).map_err(LoadError::Broken)?;
    }
    let last = events.last().ok_or(LoadError::Empty)?.seq;
    let upto = match ledger.open_turn() {
        Some(turn) => Seq::new(turn.started().get() - 1).ok_or(LoadError::Empty)?,
        None => last,
    };
    plan_upto(events, upto, None)
}

pub(super) fn fork_plan_at(
    parent: &SessionId,
    events: &[Event],
    call: CallId,
) -> Result<ForkPlan, LoadError> {
    let mut ledger = Ledger::for_session(parent.clone());
    for event in events {
        ledger.append(event).map_err(LoadError::Broken)?;
    }
    let last = events.last().ok_or(LoadError::Empty)?.seq;
    let not_waiting = || {
        LoadError::Broken(LedgerError {
            seq: last,
            why: format!("{call} is not a call waiting for a result in the running turn"),
        })
    };
    let turn = ledger.open_turn().ok_or_else(not_waiting)?;
    if !ledger.pending_calls().contains(&call) {
        return Err(not_waiting());
    }
    let upto = ledger.last_reply().ok_or_else(not_waiting)?;
    plan_upto(events, upto, Some(turn))
}

fn plan_upto(events: &[Event], upto: Seq, within: Option<TurnId>) -> Result<ForkPlan, LoadError> {
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
        persona: reference.persona().map(str::to_string),
        preset: reference.preset().map(str::to_string),
        within,
    })
}

/// 以前的 `Session::fork` / `fork_within`（`placeholder` 有没有分两种），交回会话和动作。
#[allow(clippy::too_many_arguments, reason = "原样抄来")]
pub(super) fn fork_on(
    session: SessionId,
    id: CommandId,
    by: By,
    at: Timestamp,
    base: &[Event],
    placeholder: Option<&str>,
    created: SessionCreated,
    policy: Policy,
    environment: Environment,
) -> Result<(Session, Vec<Action>), LoadError> {
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
    let base = Base::of(base, forked)?;
    let history = base
        .history()
        .owned_by(session.clone())
        .forked_from(base.parent.clone());
    let first = base
        .events
        .last()
        .map_or(Seq::FIRST, |event| event.seq.next());
    let ledger = continued(
        Ledger::for_session(session.clone())
            .starting_at(first)
            .heard_from(&base.ledger),
        &base,
    );
    let mut session = Session::blank(session, ledger, history, &created, policy, environment);
    let mut events = vec![session.record(at, by, Some(id.clone()), Body::SessionCreated(created))];
    if let Some((turn, placeholder)) = placeholder {
        events.extend(session.settle_inherited(at, &id, turn, placeholder));
    }
    session.accept(id, events.iter().map(|event| event.seq).collect());
    Ok((session, vec![Action::Append(events)]))
}

fn forked_of(own: &[Event]) -> Result<Forked, LoadError> {
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

/// 以前的 `Session::loader`：底子整份交过来，第一块自己的日志进来才放。
pub(super) fn loader(
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
            ledger = continued(ledger.heard_from(&of.ledger), &of);
            (Some((of.parent, of.from)), base)
        }
        None => (None, Vec::new()),
    };
    Ok(Loader {
        session,
        reports,
        parent,
        kept: base,
        shells: Default::default(),
        ledger,
        replay: Replay::default(),
        duty: Duty::default(),
        naming: Naming::default(),
        reference: Reference::default(),
        sight: Sight::default(),
        grants: Grants::default(),
        rounds: Rounds::default(),
        busy: crate::waits::Busy::default(),
        chain: crate::session::chain::Chain::default(),
        history: History::whole(),
        noted: Seq::FIRST,
    })
}

/// 以前的 `Session::load_forked`：整份当一块喂。
pub(super) fn load_forked(
    session: SessionId,
    base: &[Event],
    events: Vec<Event>,
    at: Timestamp,
    policy: Policy,
    environment: Environment,
) -> Result<(Session, Vec<Action>), LoadError> {
    let mut loader = loader(
        session,
        Some(base.to_vec()),
        &events,
        policy.reports.clone(),
    )?;
    loader.feed(events)?;
    loader.finish(at, policy, environment)
}
