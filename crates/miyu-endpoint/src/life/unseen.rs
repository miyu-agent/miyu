//! 结束了、还没看过的人生（施工 LF-14，`docs/blueprint/life.md`「结束了、还没看过」；项目主人 2026-11-17 定：人要知道她
//! 结束了自己的人生）：
//!
//! - 记：她自己结束的（`end_life`），那一刻没有头开着这个会话的，记一条（会话、人格、时刻）。主人结束的不记：是他自己做的；
//!   有头正开着的不记：人就在看着。
//! - 给头：`life.list` 多回一格 `ended_unseen`，头照它画顶上的提示、入口上的记号；结束那一刻另有通知 `life.ended`（`finish.rs`）。
//! - 清：有头打开那个会话（`session.viewing`）、头说一声看过了（`life.seen`）、会话删了。清掉了推一条通知 `life.seen`，别的头
//!   跟着收起提示。只清那一个会话的。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_store::schedules::Unseen;

use crate::Core;
use crate::methods::params;
use crate::refusal::Refusal;
use crate::schedules::ScheduleError;
use crate::wire::Request;

/// 看过了的通知。
const SEEN: &str = "life.seen";

/// 她自己结束了会话 `session` 上的人生：没有头正开着它的，记成还没看过。记不下的记一行，不碍结束。
pub(super) fn mark(core: &Core, session: &SessionId, persona: Option<String>) {
    if core.viewing.ids().contains(session) {
        return;
    }
    let one = Unseen {
        session: session.clone(),
        persona,
        at: core.schedules().now(),
    };
    if let Err(error) = core.schedules().mark_unseen(&one) {
        tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "ended life not marked unseen");
    }
}

/// 会话 `session` 看过了（有头打开了它、头说看过了、它删了）：记着的清掉，推一条 `life.seen`。没记着的什么都不做。
pub(crate) fn seen(core: &Core, session: &SessionId) {
    match core.schedules().seen(session) {
        Ok(true) => core.schedules().notices().send(
            SEEN,
            core.schedules().now(),
            json!({"session": session.as_str()}),
        ),
        // 没有唤醒表的（从没开过人生、定时）、表用不了的：没有记着的。
        Ok(false) | Err(ScheduleError::Unknown | ScheduleError::Unusable) => {}
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "ended life not marked seen");
        }
    }
}

/// `life.list` 的 `ended_unseen`：一项 `{"session", "persona"?, "name"?, "at"}`，结束得早的在前。读不了的记一行、当没有。
pub(super) async fn listed(core: &Arc<Core>, language: &str) -> Vec<Value> {
    let unseen = match core.schedules().unseen() {
        Ok(unseen) => unseen,
        Err(ScheduleError::Unknown | ScheduleError::Unusable) => Vec::new(),
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "unseen ended lives not read");
            Vec::new()
        }
    };
    let mut listed = Vec::with_capacity(unseen.len());
    for one in unseen {
        let mut item = json!({"session": one.session.as_str(), "at": one.at});
        if let Some(persona) = one.persona {
            item["name"] = json!(super::name(core, &persona, language).await);
            item["persona"] = json!(persona);
        }
        listed.push(item);
    }
    listed
}

/// `life.seen` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeenParams {
    session: String,
}

/// `life.seen {session}`：头说这一段看过了（人收起了提示）。回 `{}`；本来就没记着的也回 `{}`。
///
/// # Errors
///
/// 参数不对、会话编号写法不对：`bad_params`。
pub(crate) fn said(core: &Core, request: &Request) -> Result<Value, Refusal> {
    let SeenParams { session } = params(request)?;
    let session = SessionId::parse(&session).map_err(|_| Refusal::BAD_PARAMS)?;
    seen(core, &session);
    Ok(json!({}))
}
