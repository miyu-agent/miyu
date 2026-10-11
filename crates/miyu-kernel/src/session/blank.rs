//! 刚造、还没记 `session.created` 的会话（施工 C-8 上从 `session.rs` 挪出来：那一份到了 500 行）：造会话、分叉共用。

use super::Session;
use super::policy::Policy;
use crate::event::SessionCreated;
use crate::facts::Environment;
use crate::history::History;
use crate::id::SessionId;
use crate::ledger::Ledger;

impl Session {
    /// 一个刚造、还没记 `session.created` 的会话：账本、有效历史照交进来的（造会话是空的，分叉出来的带底子，施工 C-8 上）。
    pub(super) fn blank(
        session: SessionId,
        ledger: Ledger,
        history: History,
        created: &SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> Session {
        Session {
            ledger,
            history,
            id: session,
            unstored: Vec::new(),
            stored: None,
            waiting: Vec::new(),
            recent: super::recent::Recent::default(),
            policy: super::policy::Held::new(policy),
            environment,
            permission: created.permission.clone(),
            effective: created.permission.clone(),
            turn: None,
            last_request: None,
            closing: Vec::new(),
            restoring: None,
            reading: None,
            limits: None,
            oneshot: created.oneshot,
            watched: false,
            deferred: Vec::new(),
            restarting: false,
            duty: super::report::Duty::default(),
            meta: super::meta::Meta::default(),
            recapping: None,
            naming: super::title::Naming::default(),
            titling: None,
            prepare: super::prepare::Prepare::default(),
            reference: super::configure::Reference::default(),
            sight: super::sight::Sight::default(),
            grants: super::grants::Grants::default(),
            rounds: super::seal::Rounds::default(),
            told_todos: Vec::new(),
        }
    }
}
