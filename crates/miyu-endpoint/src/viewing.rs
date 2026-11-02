//! 哪个头开着哪个会话（施工 HV-1，2026-11-02 项目主人定「核心记谁开着哪个会话」）：头切会话、新开、刚起来时发
//! `session.viewing {session}`，核心照连接记一份，连接断了就忘。会话列表那一项至少一个头开着的写 `viewed: true`，变了推
//! `sessions.changed`：后台会话答完、在问的通知只由开着它的那个头出，别的头照 `viewed` 不再另弹，免得两个头弹两遍。

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
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

/// 每个连接开着哪个会话。
#[derive(Default)]
pub(crate) struct Viewing {
    /// 连接的座位号 → 它开着的会话；没开着的不在里面。
    seats: Mutex<HashMap<u64, SessionId>>,
    /// 下一个座位号。
    next: AtomicU64,
}

impl Viewing {
    /// 一个连接的座位：连接起来时领，断了丢掉就忘了它开着的那个。
    pub(crate) fn seat(core: &Arc<Core>) -> Seat {
        Seat {
            core: Arc::clone(core),
            id: core.viewing.next.fetch_add(1, Ordering::Relaxed),
        }
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
}

impl Seat {
    /// 这个连接开着 `session`（空的是哪个都没开）：「有没有头开着」变了的会话，列表那一项推一次。
    pub(crate) fn view(&self, session: Option<SessionId>) {
        for changed in self.core.viewing.set(self.id, session) {
            self.core.listing.changed(&self.core, changed);
        }
    }
}

impl Drop for Seat {
    fn drop(&mut self) {
        self.view(None);
    }
}

#[cfg(test)]
mod tests;
