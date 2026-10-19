//! 终端会话（施工 8-33，设计 `22-命令行.md` 第七节，2026-10-20 项目主人定）：在 shell 里说的话进的那个一直续下去的会话。
//! 指针按账号记在 `home/<账号>/terminal.json`（`miyu_store::terminal`）。哪个算、什么时候换新的由核心定，不在头：头去建
//! 会话、定规则，是向上依赖。
//!
//! - `session.terminal {cwd?}`：交回终端会话。指着的还在、是这个账号自己的本机主会话，就是它；闲置超过
//!   `terminal.idle_new_after` 分钟的（0 是不换）开一个新的，回应带 `renewed: {last}`，旧的不动；没指过的、指着的删了、
//!   不算数的开一个新的，回应带 `created: true`。新开的照 `terminal.persona`、`terminal.preset`（没设的跟随默认），在 `cwd`
//!   （没写的照默认工作区）。同一时刻几个 shell 一起要的只开一个。
//! - `session.set_terminal {session}`：指到这个会话（shellhook 的 `/new`、`/sessions`）。只收这个账号自己的本机主会话。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::Timestamp;
use miyu_store::index::Row;
use miyu_store::terminal::{self, Pointer};

use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::refusal::Refusal;
use crate::sessions::{Opening, now};
use crate::settings::TerminalSettings;
use crate::wire::Request;

/// `session.terminal` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TerminalParams {
    /// 要新开时在哪个目录：shell 这时的目录，人看到的写法。没写的照默认工作区。
    #[serde(default)]
    cwd: Option<String>,
}

/// `session.set_terminal` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    session: String,
}

/// `session.terminal`：见模块的说明。
pub(crate) async fn get(
    core: &Arc<Core>,
    peer: Peer,
    account: &AccountId,
    request: &Request,
    params: TerminalParams,
) -> Result<Value, Refusal> {
    let _one = core.terminal.lock().await;
    let path = core.root.account_dir(account).join(terminal::FILE);
    let (pointer, version) = read(&path)?;
    let settings = TerminalSettings::from(&core.config().resolved().values());
    let pointed = pointer
        .session
        .as_deref()
        .and_then(|id| SessionId::parse(id).ok())
        .and_then(|id| row(core, account, &id));
    let renewed = match &pointed {
        Some(row) if !idle(row.last_active, settings.idle_new_after, now()) => {
            return Ok(json!({"session": row.id.as_str()}));
        }
        Some(row) => Some(row.last_active),
        None => None,
    };
    let who = Opening {
        owner: account.clone(),
        attended: peer.input,
        oneshot: false,
        model: None,
        venue: None,
        memory: None,
        preset: settings.preset,
        group: None,
        chosen: params.cwd.is_some(),
        profile: false,
        images: false,
    };
    let cwd = params.cwd.unwrap_or_else(|| "~".to_string());
    let persona = settings.persona.as_deref().map(Some);
    let created = core
        .sessions
        .create(core, request.id.clone(), persona, cwd, Vec::new(), who)
        .await?;
    let pointer = Pointer {
        session: Some(created.id.as_str().to_string()),
    };
    terminal::write(&path, &pointer, version.as_deref()).map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", error = %error, "terminal session not recorded");
        Refusal::INTERNAL
    })?;
    let mut reply = json!({"session": created.id.as_str()});
    match renewed {
        Some(last) => reply["renewed"] = json!({ "last": last }),
        None => reply["created"] = json!(true),
    }
    Ok(reply)
}

/// `session.set_terminal`：见模块的说明。回应 `{"session"}`。
pub(crate) async fn set(
    core: &Arc<Core>,
    account: &AccountId,
    params: SetParams,
) -> Result<Value, Refusal> {
    let id = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    if row(core, account, &id).is_none() {
        return Err(Refusal::NOT_FOUND);
    }
    let _one = core.terminal.lock().await;
    let path = core.root.account_dir(account).join(terminal::FILE);
    let (_, version) = read(&path)?;
    let pointer = Pointer {
        session: Some(id.as_str().to_string()),
    };
    terminal::write(&path, &pointer, version.as_deref()).map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", error = %error, "terminal session not recorded");
        Refusal::INTERNAL
    })?;
    Ok(json!({"session": id.as_str()}))
}

/// 读指针：读不成的（手改坏了）当没指过，记一行，下一回写成好的。
fn read(path: &std::path::Path) -> Result<(Pointer, Option<String>), Refusal> {
    match terminal::read(path) {
        Ok(read) => Ok(read),
        Err(terminal::BadFile::Shape(why)) => {
            tracing::warn!(target: "miyu::endpoint", reason = %why, "terminal session pointer unreadable");
            let version = miyu_store::config_file::read(path)
                .ok()
                .flatten()
                .map(|text| text.version);
            Ok((Pointer::default(), version))
        }
        Err(terminal::BadFile::Read(error)) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "terminal session pointer unreadable");
            Err(Refusal::INTERNAL)
        }
    }
}

/// 会话列表里 `id` 那一行，是 `account` 自己的、本机的、主会话（不是子会话、不是一次性的）才算：删了的、别人的、场所会话、
/// 子会话都没有。
fn row(core: &Core, account: &AccountId, id: &SessionId) -> Option<Row> {
    let rows = core.index_for(account)?.rows().ok()?;
    rows.get(id).cloned().filter(|row| {
        row.owner == *account && row.venue == LOCAL && row.parent.is_none() && !row.oneshot
    })
}

/// 上一次动静 `last` 到 `now` 超过 `minutes` 分钟了没有：0 是不换。
fn idle(last: Timestamp, minutes: i64, now: Timestamp) -> bool {
    minutes > 0 && now.unix_millis() - last.unix_millis() > minutes.saturating_mul(60_000)
}

#[cfg(test)]
mod tests;
