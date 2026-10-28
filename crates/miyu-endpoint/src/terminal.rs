//! 终端会话（施工 8-33，设计 `22-命令行.md` 第七节，2026-10-20 项目主人定）：在 shell 里说的话进的那个一直续下去的会话。
//! 指针按账号记在 `terminal.json`（`miyu_store::terminal`）。哪个算、什么时候换新的由核心定，不在头：头去建
//! 会话、定规则，是向上依赖。
//!
//! - `session.terminal {cwd?, fresh?}`：交回终端会话。指着的还在、是这个账号自己的本机主会话，就是它；闲置超过
//!   `terminal.idle_new_after` 分钟的（0 是不换）开一个新的，回应带 `renewed: {last}`，旧的不动；没指过的、指着的删了、
//!   不算数的开一个新的，回应带 `created: true`。新开的照 `terminal.persona`、`terminal.preset`（没设的跟随默认），在 `cwd`
//!   （没写的照默认工作区）。同一时刻几个 shell 一起要的只开一个。`fresh: true` 不管指着谁，照新开的规矩开一个、指过去，
//!   回应带 `created: true`（shellhook 的 `/new`，施工 8-33 再补：怎么开归核心，头不照 `terminal.*` 再拼一遍）。
//! - `session.set_terminal {session}`：指到这个会话（shellhook 的 `/sessions`）。只收这个账号自己的本机主会话。
//! - 指过去（`set_terminal`、新开的）的那一刻记在指针上，算这个终端会话的一次动静：闲置照会话的最后一次动静和它里晚的那个算
//!   （施工 8-33 三补，2026-10-21 项目主人定：主动切过去的不该再被闲置规则换掉）。
//! - 按终端分开（施工 T-22，2026-10-23 项目主人定：不同终端不同会话，真的对话了才开新的）：两个方法都多收 `terminal`（头生成的
//!   键，1 到 128 个字、没有控制字符），带了的照「账号 + 键」各记一个指针，不带的照旧用原来那一个。`session.terminal` 多收
//!   `create`（不写是真）：假的只问不开，没有、指着的不在了、闲置超时的回 `{"session": null}`，不新开、不挪指针；和 `fresh`
//!   一起写的 `bad_params`。键各记最后一次用到的时刻，写的时候顺手清掉 7 天没用的，最多留 64 个（`miyu_store::terminal`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::Timestamp;
use miyu_store::index::Row;
use miyu_store::terminal::{self, Keyed, Pointer, Pointers};

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
    /// 不管指着谁，开一个新的指过去（施工 8-33 再补）。
    #[serde(default)]
    fresh: bool,
    /// 哪个终端（施工 T-22）：不写的用原来那一个指针。
    #[serde(default)]
    terminal: Option<String>,
    /// 假的只问不开（施工 T-22）：不写是真。
    #[serde(default)]
    create: Option<bool>,
}

/// `session.set_terminal` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    session: String,
    /// 哪个终端（施工 T-22）：不写的改原来那一个指针。
    #[serde(default)]
    terminal: Option<String>,
}

/// `session.terminal`：见模块的说明。
pub(crate) async fn get(
    core: &Arc<Core>,
    peer: Peer,
    account: &AccountId,
    request: &Request,
    params: TerminalParams,
) -> Result<Value, Refusal> {
    let key = key(params.terminal)?;
    let create = params.create.unwrap_or(true);
    if params.fresh && !create {
        return Err(Refusal::BAD_PARAMS);
    }
    let _one = core.terminal.lock().await;
    let path = core.root.path().join(terminal::FILE);
    let (mut pointers, version) = read(&path)?;
    let pointer = current(&pointers, key.as_deref());
    let settings = TerminalSettings::from(&core.mine(account).config().resolved().values());
    let pointed = pointer
        .session
        .as_deref()
        .and_then(|id| SessionId::parse(id).ok())
        .and_then(|id| row(core, account, &id));
    let renewed = match &pointed {
        _ if params.fresh => None,
        Some(row) if !idle(touched(row, &pointer), settings.idle_new_after, now()) => {
            // 按键的记一下这一次用到了：清久没用的照它。
            if let Some(key) = &key {
                store(&mut pointers, Some(key), pointer.clone());
                save(&path, &pointers, version.as_deref())?;
            }
            return Ok(json!({"session": row.id.as_str()}));
        }
        Some(row) => Some(row.last_active),
        None => None,
    };
    if !create {
        return Ok(json!({"session": null}));
    }
    let who = Opening {
        owner: account.clone(),
        attended: peer.input,
        oneshot: false,
        model: None,
        venue: None,
        memory: None,
        preset: settings.preset,
        group: None,
        profile: false,
        images: false,
        titles: true,
        // 终端集成开的（施工 8-33 四补）：会话列表照它分得出来。
        origin: Some("terminal".to_string()),
        confined: false,
    };
    let cwd = params.cwd.unwrap_or_else(|| "~".to_string());
    let persona = settings.persona.as_deref().map(Some);
    let created = core
        .sessions
        .create(core, request.id.clone(), persona, cwd, Vec::new(), who)
        .await?;
    let pointer = Pointer {
        session: Some(created.id.as_str().to_string()),
        at: Some(now()),
    };
    store(&mut pointers, key.as_deref(), pointer);
    save(&path, &pointers, version.as_deref())?;
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
    let key = key(params.terminal)?;
    if row(core, account, &id).is_none() {
        return Err(Refusal::NOT_FOUND);
    }
    let _one = core.terminal.lock().await;
    let path = core.root.path().join(terminal::FILE);
    let (mut pointers, version) = read(&path)?;
    let pointer = Pointer {
        session: Some(id.as_str().to_string()),
        at: Some(now()),
    };
    store(&mut pointers, key.as_deref(), pointer);
    save(&path, &pointers, version.as_deref())?;
    Ok(json!({"session": id.as_str()}))
}

/// 终端的键（施工 T-22）：1 到 128 个字、没有控制字符；不合的 `bad_params`。不写的没有。
fn key(terminal: Option<String>) -> Result<Option<String>, Refusal> {
    match terminal {
        None => Ok(None),
        Some(key)
            if (1..=128).contains(&key.chars().count()) && !key.chars().any(char::is_control) =>
        {
            Ok(Some(key))
        }
        Some(_) => Err(Refusal::BAD_PARAMS),
    }
}

/// 这个键（没有的是原来那一个）现在的指针；还没记过的是空的。
fn current(pointers: &Pointers, key: Option<&str>) -> Pointer {
    match key {
        None => pointers.main.clone(),
        Some(key) => pointers
            .terminals
            .get(key)
            .map(|keyed| keyed.pointer.clone())
            .unwrap_or_default(),
    }
}

/// 把这个键（没有的是原来那一个）的指针换成 `pointer`；按键的记下这一刻用到了，顺手清久没用的。
fn store(pointers: &mut Pointers, key: Option<&str>, pointer: Pointer) {
    match key {
        None => pointers.main = pointer,
        Some(key) => {
            let used = now();
            pointers
                .terminals
                .insert(key.to_string(), Keyed { pointer, used });
            pointers.prune(used);
        }
    }
}

/// 写回：写不成的记一行，`internal_error`。
fn save(path: &std::path::Path, pointers: &Pointers, version: Option<&str>) -> Result<(), Refusal> {
    terminal::write(path, pointers, version).map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", error = %error, "terminal session not recorded");
        Refusal::INTERNAL
    })
}

/// 终端会话最后一次动静：会话自己的，和指过去的那一刻里晚的那个。
fn touched(row: &Row, pointer: &Pointer) -> Timestamp {
    pointer
        .at
        .filter(|at| *at > row.last_active)
        .unwrap_or(row.last_active)
}

/// 读指针：读不成的（手改坏了）当没指过，记一行，下一回写成好的。
fn read(path: &std::path::Path) -> Result<(Pointers, Option<String>), Refusal> {
    match terminal::read(path) {
        Ok(read) => Ok(read),
        Err(terminal::BadFile::Shape(why)) => {
            tracing::warn!(target: "miyu::endpoint", reason = %why, "terminal session pointer unreadable");
            let version = miyu_store::config_file::read(path)
                .ok()
                .flatten()
                .map(|text| text.version);
            Ok((Pointers::default(), version))
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
    let rows = core.index_for(account).rows().ok()?;
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
