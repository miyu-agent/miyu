//! 哪个头开着哪个会话（施工 HV-1，2026-11-02 项目主人定「核心记谁开着哪个会话」）：头切会话、新开、刚起来时发
//! `session.viewing {session}`，核心照连接记一份，连接断了就忘。会话列表那一项至少一个头开着的写 `viewed: true`，变了推
//! `sessions.changed`：后台会话答完、在问的通知只由开着它的那个头出，别的头照 `viewed` 不再另弹，免得两个头弹两遍。
//!
//! 谁来发后台通知（施工 TR-1，托盘 `miyu-applet` 的蓝图 `app.md` 第 5 条）：托盘这样浏览器关了也要弹的头调 `notices.claim`，
//! 核心照同一个座位记，连接断了就放。「有没有人认领」变了往 `notices` 流推 `notices.claimed`（`schedules/notices.rs`），
//! 订阅的回应照 [`Viewing::claimed`] 写；有人认领时别的头不再为没人开着的会话、`notice` 弹，免得和托盘弹两遍。

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;
use serde_json::Value;

use miyu_kernel::id::SessionId;

use crate::Core;
use crate::refusal::Refusal;

/// `session.viewing` 的参数：`session` 是开着的会话，`null`、不写是哪个都没开。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    #[serde(default)]
    session: Option<String>,
}

/// `session.viewing` 要开着的会话：写法不对的 `bad_params`，没有这个会话的 `session_not_found`。
pub(crate) async fn requested(
    core: &Arc<Core>,
    params: Value,
) -> Result<Option<SessionId>, Refusal> {
    let params: Params = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    let Some(session) = params.session else {
        return Ok(None);
    };
    let session = SessionId::parse(&session).map_err(|_| Refusal::BAD_PARAMS)?;
    if core.sessions.loaded(&session).await.is_none() && !core.sessions.exists(core, &session).await
    {
        return Err(Refusal::NOT_FOUND);
    }
    Ok(Some(session))
}

/// `notices.claim` 不收参数：不写、`null`、`{}` 都行，带了什么的 `bad_params`。
///
/// # Errors
///
/// 带了参数的 `bad_params`。
pub(crate) fn claim_params(params: &Value) -> Result<(), Refusal> {
    match params {
        Value::Null => Ok(()),
        Value::Object(params) if params.is_empty() => Ok(()),
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// 每个连接开着哪个会话，几个连接认领了发后台通知。
#[derive(Default)]
pub(crate) struct Viewing {
    /// 连接的座位号 → 它开着的会话；没开着的不在里面。
    seats: Mutex<HashMap<u64, SessionId>>,
    /// 认领了发后台通知的连接有几个（施工 TR-1）：一个连接只算一次（[`Seat`] 记着认没认过）。
    claimants: Mutex<usize>,
    /// 下一个座位号。
    next: AtomicU64,
}

impl Viewing {
    /// 一个连接的座位：连接起来时领，断了丢掉就忘了它开着的那个。
    pub(crate) fn seat(core: &Arc<Core>) -> Seat {
        Seat {
            core: Arc::clone(core),
            id: core.viewing.next.fetch_add(1, Ordering::Relaxed),
            claimed: AtomicBool::new(false),
        }
    }

    /// 这时有没有连接认领了发后台通知（施工 TR-1）：`notices` 流的订阅回应照它写 `claimed`。
    pub(crate) fn claimed(&self) -> bool {
        *self.claims() > 0
    }

    /// 认领的多一个（`more`）或少一个：从没有变成有、从有变成没有的，往 `notices` 推一次。推在锁里，几个连接同时来去，
    /// 推出去的先后和计数变的先后一样，最后一条总是这时的样子。
    fn count(&self, more: bool, notices: &crate::schedules::notices::Notices) {
        let mut claims = self.claims();
        let before = *claims > 0;
        *claims = match more {
            true => *claims + 1,
            false => claims.saturating_sub(1),
        };
        let after = *claims > 0;
        if before != after {
            notices.claimed(after);
        }
    }

    fn claims(&self) -> std::sync::MutexGuard<'_, usize> {
        self.claimants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// 这时至少一个头开着的会话。
    pub(crate) fn ids(&self) -> BTreeSet<SessionId> {
        self.lock().values().cloned().collect()
    }

    /// 座位 `seat` 改成开着 `session`（空的是哪个都没开）：交回「有没有头开着」因此变了的会话。
    fn set(&self, seat: u64, session: Option<SessionId>) -> Vec<SessionId> {
        let mut seats = self.lock();
        let before = seats.get(&seat).cloned();
        if before == session {
            return Vec::new();
        }
        match &session {
            Some(session) => seats.insert(seat, session.clone()),
            None => seats.remove(&seat),
        };
        let viewed = |session: &SessionId| seats.values().any(|open| open == session);
        let left = before.filter(|before| !viewed(before));
        let entered = session.filter(|session| {
            seats
                .iter()
                .all(|(at, open)| *at == seat || open != session)
        });
        left.into_iter().chain(entered).collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, SessionId>> {
        self.seats.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 一个连接的座位（[`Viewing::seat`]）。
pub(crate) struct Seat {
    core: Arc<Core>,
    id: u64,
    /// 这个连接认领了发后台通知（施工 TR-1）：再调一遍不重数，断了照它放。
    claimed: AtomicBool,
}

impl Seat {
    /// 这个连接开着 `session`（空的是哪个都没开）：「有没有头开着」变了的会话，列表那一项推一次。
    pub(crate) fn view(&self, session: Option<SessionId>) {
        for changed in self.core.viewing.set(self.id, session) {
            self.core.listing.changed(&self.core, changed);
        }
    }

    /// 这个连接认领发后台通知（`notices.claim`）：认过的不再数。
    pub(crate) fn claim(&self) {
        if !self.claimed.swap(true, Ordering::Relaxed) {
            self.core
                .viewing
                .count(true, self.core.schedules().notices());
        }
    }
}

impl Drop for Seat {
    fn drop(&mut self) {
        self.view(None);
        if self.claimed.load(Ordering::Relaxed) {
            self.core
                .viewing
                .count(false, self.core.schedules().notices());
        }
    }
}

#[cfg(test)]
mod tests;
