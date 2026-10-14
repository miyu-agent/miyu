//! 斜杠命令由核心解析（施工 O-6，`docs/designs/04-核心协议.md` P4，`docs/blueprint/protocol.md`「`command.run`」）：头把人打的
//! 原文交过来，核心认命令、判谁能用、执行、记一条 `command.ran`，回执的那一句照连接的语言写好交回去。
//!
//! 认哪些命令照登记表（施工 O-6 三补，[`registry`]）：核心自己的 `/clear`（别名 `/reset`）、`/stop`、`/workspace` 在
//! `basic.rs`；记忆的 `/remember`、`/dream` 在 `memory/slash.rs`；长期目标的 `/goal` 在 `goal.rs`。各自写名字、别名、跟不跟字、
//! 在不在后台答、谁能用、怎么执行；这里只认、判身份、记下、写回执，不再写死名字。
//!
//! 命令本身照请求的编号交给内核，内核照编号只生效一次；`command.ran` 用派生的编号 `<编号>/ran` 记，执行了的才记。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::{By, Role};
use miyu_kernel::session::{Command, Outcome};
use miyu_session::Handle;
use miyu_store::human::Human;

use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::refusal::Refusal;
use crate::sessions::admin;
use crate::venues::{self, AsParams};
use crate::wire::Request;

pub(crate) use registry::{Call, Commands, Ran, Running, Slash};

/// `command.run` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunParams {
    session: String,
    text: String,
    #[serde(default, rename = "as")]
    as_external: Option<AsParams>,
    /// 头所在的目录：只用来接 `/workspace` 后面相对的路径（施工 9-7 下）。
    #[serde(default)]
    cwd: Option<String>,
}

/// 这一条 `command.run` 在后台答不答（施工 R-7 补，`methods::answered_later` 问它）：照原文认出的命令问
/// [`Slash::later`]；认不出的、参数不对的照旧按顺序答，由 [`run`] 拒。
pub(crate) fn answered_later(core: &Core, request: &Request) -> bool {
    request
        .params
        .get("text")
        .and_then(Value::as_str)
        .and_then(|text| parse(&core.commands, text).ok())
        .is_some_and(|(slash, _)| slash.later())
}

/// 核心起来时的登记表：核心自己的、记忆的、长期目标的（施工 O-6 三补）。
pub(crate) fn builtin() -> Commands {
    let mut commands = Commands::default();
    basic::register(&mut commands);
    crate::memory::slash::register(&mut commands);
    goal::register(&mut commands);
    commands
}

/// 回执那一句：`core/human/<语言>.json` 的键，和要填的字。
pub(crate) struct Said {
    /// 键。
    pub(crate) key: &'static str,
    /// 要填的字。
    pub(crate) fields: Vec<(&'static str, String)>,
}

impl Said {
    /// 不用填字的一句。
    pub(crate) fn plain(key: &'static str) -> Said {
        Said {
            key,
            fields: Vec::new(),
        }
    }
}

/// `command.run`：认命令、判谁能用、执行、记下，交回 `{"command", "events", "said"}`。
pub(crate) async fn run(
    core: &Arc<Core>,
    peer: &Peer,
    id: &CommandId,
    params: RunParams,
) -> Result<Value, Refusal> {
    let (slash, rest) = parse(&core.commands, &params.text)?;
    if params
        .cwd
        .as_deref()
        .is_some_and(|cwd| !workspace::head_cwd_ok(cwd))
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let noted = CommandId::parse(&format!("{id}/ran")).map_err(|_| Refusal::BAD_PARAMS)?;
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session).await?;
    let handle = found.handle.clone();
    let local = handle.venue().as_str() == LOCAL;
    let by = match (local, params.as_external) {
        (true, None) => admin(core),
        (true, Some(_)) => return Err(Refusal::BAD_PARAMS),
        (false, None) => return Err(Refusal::VENUE_SESSION),
        (false, Some(speaking)) => venues::speaker(core, handle.venue(), handle.owner(), speaking)?,
    };
    if let Err(refused) = allowed(core, slash.as_ref(), &handle, &by) {
        let why = why::why(core, peer, &handle, &refused).await;
        return Err(refused.because(why));
    }
    let call = Call {
        core,
        peer,
        session: &session,
        found: &found,
        id,
        by: &by,
        rest,
        head: params.cwd.as_deref(),
    };
    let Ran { mut events, said } = slash.run(call).await?;
    let note = Command::Ran {
        text: params.text.clone(),
        command: slash.name().to_string(),
    };
    events.extend(accepted(
        command(core, &session, &handle, &noted, &by, note).await?,
    )?);
    let said = words(core, peer, &said).await;
    Ok(json!({"command": slash.name(), "events": events, "said": said}))
}

/// 原文的头一个词是命令名：开头的空白不算，`/` 开头，名字紧跟着 `/`、到空白为止。照登记表认，交回命令和后面跟的字（去掉
/// 前后空白）。不是 `/` 开头的参数不对，认不出的 `unknown_command`。
fn parse<'a>(
    commands: &'a Commands,
    text: &'a str,
) -> Result<(&'a Arc<dyn Slash>, &'a str), Refusal> {
    let after = text
        .trim_start()
        .strip_prefix('/')
        .ok_or(Refusal::BAD_PARAMS)?;
    let (name, rest) = after.split_once(char::is_whitespace).unwrap_or((after, ""));
    let slash = commands.find(name).ok_or(Refusal::UNKNOWN_COMMAND)?;
    Ok((slash, rest.trim()))
}

/// `by` 在这个会话里能不能用 `slash`（施工 O-6 补收成一处，`command.run` 和 `command.catalog` 共用）：先判身份
/// （[`may_run`]），再问命令自己（[`Slash::allowed`]：主人本人才行、要开着记忆这些）。
fn allowed(core: &Core, slash: &dyn Slash, handle: &Handle, by: &By) -> Result<(), Refusal> {
    if !may_run(by) {
        return Err(Refusal::COMMAND_NOT_ALLOWED);
    }
    slash.allowed(core, handle, by)
}

/// 谁能用（`18-通讯平台.md` 第十二节）：本人（本机的头、私聊里对应表认出的本人），对应表里有的外部身份（群里的主人），场所里
/// 管理的人。
fn may_run(by: &By) -> bool {
    match by {
        By::Person(_) => true,
        By::External(external) => {
            external.account.is_some() || external.role == Some(Role::Manager)
        }
        _ => false,
    }
}

/// 交给会话 `session` 一个命令，等回应。会话停了的从表里拿掉。
async fn command(
    core: &Core,
    session: &SessionId,
    handle: &Handle,
    id: &CommandId,
    by: &By,
    command: Command,
) -> Result<Outcome, Refusal> {
    match handle.command(id.clone(), by.clone(), command).await {
        Ok(outcome) => Ok(outcome),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 接受了的交回序号；拒绝的照内核的原因说。
fn accepted(outcome: Outcome) -> Result<Vec<u64>, Refusal> {
    match outcome {
        Outcome::Accepted { events } => Ok(events.iter().map(|seq| seq.get()).collect()),
        Outcome::Rejected { reason, .. } => Err(Refusal::kernel(reason)),
        _ => Err(Refusal::INTERNAL),
    }
}

/// 回执那一句，照这个连接的语言；资源读不出来的是空的，记一行运行日志。
async fn words(core: &Core, peer: &Peer, said: &Said) -> String {
    let resources = core.resources.clone();
    let language = peer.language.to_string();
    let loaded = tokio::task::spawn_blocking(move || Human::load(&resources, &language)).await;
    let fields: Vec<(&str, &str)> = said
        .fields
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    match loaded {
        Ok(Ok(human)) => Words::sentence(&human, said.key, &fields).unwrap_or_default(),
        _ => {
            tracing::warn!(target: "miyu::endpoint", "command receipt words not read");
            String::new()
        }
    }
}

mod basic;
mod catalog;
mod goal;
mod registry;
#[cfg(test)]
mod tests;
mod why;
mod workspace;

pub(crate) use catalog::catalog;
