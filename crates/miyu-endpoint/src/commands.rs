//! 斜杠命令由核心解析（施工 O-6，`docs/designs/04-核心协议.md` P4，`docs/blueprint/protocol.md`「`command.run`」）：头把人打的
//! 原文交过来，核心认命令、判谁能用、执行、记一条 `command.ran`，回执的那一句照连接的语言写好交回去。
//!
//! 认哪些命令照登记表（施工 O-6 三补，[`registry`]）：核心自己的 `/clear`（别名 `/reset`）、`/stop`、`/workspace` 在
//! `basic.rs`；记忆的 `/remember`、`/dream` 在 `memory/slash.rs`；长期目标的 `/goal` 在 `goal.rs`；看、清待办的 `/todo` 在 `todo.rs`（施工 D-3 再补）。各自写名字、别名、跟不跟字、
//! 在不在后台答、谁能用、怎么执行；这里只认、判身份、记下、写回执，不再写死名字。
//!
//! 命令本身照请求的编号交给内核，内核照编号只生效一次；`command.ran` 用派生的编号 `<编号>/ran` 记，执行了的才记。
//!
//! 登记表里没有的名字照模板命令认（施工 X-14 上，[`templates`]）：属主家目录、装了的包里的 `commands/<名字>.md`，展开成一段话
//! 发出去。扩展整页的命令（施工 X-15 下，[`pages`]）也在那里认：只叫头打开那一页。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_config::package::Access;
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
use crate::settings::CommandSettings;
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
    /// 本机会话里说命令的人（施工 Y-2 下下上）：照连接的人，不从参数读。没填的（不经 [`run`] 的）照旧是管理员。
    #[serde(skip)]
    person: Option<By>,
    /// 前缀要变了时人选的（施工 OV-1 下，`sessions/prefix.rs`）：开新一轮的命令（模板、提示词、设定和继续长期目标）照它。
    #[serde(default)]
    prefix: Option<crate::sessions::prefix::Choice>,
}

/// 这一条 `command.run` 在后台答不答（施工 R-7 补，`methods::answered_later` 问它）：照原文认出的命令问
/// [`Slash::later`]；认不出的、参数不对的照旧按顺序答，由 [`run`] 拒。
pub(crate) fn answered_later(core: &Core, request: &Request) -> bool {
    let Some((name, _)) = request
        .params
        .get("text")
        .and_then(Value::as_str)
        .and_then(|text| split(text).ok())
    else {
        return false;
    };
    match core.commands.find_present(core, name) {
        Some(slash) => slash.later(),
        // 扩展声明的命令要等它的程序答（施工 X-14 下）：同 `/dream` 在后台答。
        None => declared::named(core, name).is_some(),
    }
}

/// 核心起来时的登记表：核心自己的、记忆的、长期目标的（施工 O-6 三补）。
pub(crate) fn builtin() -> Commands {
    let mut commands = Commands::default();
    basic::register(&mut commands);
    crate::memory::slash::register(&mut commands);
    goal::register(&mut commands);
    looping::register(&mut commands);
    todo::register(&mut commands);
    commands
}

/// 等扩展的命令、MCP 服务的提示词答多久（施工 X-14 下；X-39 补起提示词也最多等这么久）。
pub(crate) const ANSWER_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

/// 回执那一句：`core/human/<语言>.json` 的键，和要填的字。
pub(crate) struct Said {
    /// 键。
    pub(crate) key: &'static str,
    /// 要填的字。
    pub(crate) fields: Vec<(&'static str, String)>,
    /// 照这个连接的语言拼成的几段（施工 R-22 再补）：每段是一句，照 `commands/joiner` 连起来填进同名的字。有的项是 0 不写时，
    /// 不用把每种组合都开成一个键。
    pub(crate) lists: Vec<(&'static str, Vec<Said>)>,
}

impl Said {
    /// 不用填字的一句。
    pub(crate) fn plain(key: &'static str) -> Said {
        Said::with(key, Vec::new())
    }

    /// 填字的一句。
    pub(crate) fn with(key: &'static str, fields: Vec<(&'static str, String)>) -> Said {
        Said {
            key,
            fields,
            lists: Vec::new(),
        }
    }
}

/// `command.run`：认命令、判谁能用、执行、记下，交回 `{"command", "events", "said"}`。登记表里没有的照模板命令
/// （[`templates::run`]）。
pub(crate) async fn run(
    core: &Arc<Core>,
    (caller, peer): (&crate::hello::Caller, &Peer),
    id: &CommandId,
    mut params: RunParams,
) -> Result<Value, Refusal> {
    params.person = Some(crate::sessions::person(core, caller));
    let (name, rest) = split(&params.text)?;
    if params
        .cwd
        .as_deref()
        .is_some_and(|cwd| !workspace::head_cwd_ok(cwd))
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let Some(slash) = core.commands.find_present(core, name) else {
        if name.is_empty() {
            return Err(Refusal::UNKNOWN_COMMAND);
        }
        // 名字里有 `:` 的是 MCP 服务的提示词（施工 X-39）：模板、内置命令的名字里都没有。
        if name.contains(':') {
            return prompts::run(core, peer, id, &params, (name, rest)).await;
        }
        return templates::run(core, peer, id, &params, (name, rest)).await;
    };
    let noted = CommandId::parse(&format!("{id}/ran")).map_err(|_| Refusal::BAD_PARAMS)?;
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session).await?;
    let handle = found.handle.clone();
    let by = speaker(core, &handle, params.person.as_ref(), params.as_external)?;
    if let Err(refused) = allowed(core, slash.as_ref(), &handle, &by) {
        if access(core, slash.name(), None) == Access::Off {
            return Err(turned_off(core, peer, slash.name()).await);
        }
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
        prefix: params.prefix,
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

/// 说命令的是谁：本机的会话是连上来的那个人 `person`（施工 Y-2 下下上，原来一律是管理员；不收 `as`），场所会话照 `as` 认出
/// 平台上的人（不带的 `venue_session`）。
fn speaker(
    core: &Core,
    handle: &Handle,
    person: Option<&By>,
    speaking: Option<AsParams>,
) -> Result<By, Refusal> {
    match (handle.venue().as_str() == LOCAL, speaking) {
        (true, None) => Ok(person.cloned().unwrap_or_else(|| admin(core))),
        (true, Some(_)) => Err(Refusal::BAD_PARAMS),
        (false, None) => Err(Refusal::VENUE_SESSION),
        (false, Some(speaking)) => venues::speaker(core, handle, speaking),
    }
}

/// 原文的头一个词是命令名：开头的空白不算，`/` 开头，名字紧跟着 `/`、到空白为止。交回名字和后面跟的字（去掉前后空白）。
/// 不是 `/` 开头的参数不对。
fn split(text: &str) -> Result<(&str, &str), Refusal> {
    let after = text
        .trim_start()
        .strip_prefix('/')
        .ok_or(Refusal::BAD_PARAMS)?;
    let (name, rest) = after.split_once(char::is_whitespace).unwrap_or((after, ""));
    Ok((name, rest.trim()))
}

/// `by` 在这个会话里能不能用 `slash`（施工 O-6 补收成一处，`command.run` 和 `command.catalog` 共用）：先照这条命令谁能用判身份
/// （[`access`]、[`passes`]），再问命令自己（[`Slash::allowed`]：主人本人才行、要开着记忆这些）。
fn allowed(core: &Core, slash: &dyn Slash, handle: &Handle, by: &By) -> Result<(), Refusal> {
    if !passes(access(core, slash.name(), None), by) {
        return Err(Refusal::COMMAND_NOT_ALLOWED);
    }
    slash.allowed(core, handle, by)
}

/// 命令 `name` 谁能用（施工 O-6 六补，`slash.md`「谁能用」）：系统配置 `commands.<名字>.access` 写了的照它，没写的照出厂值
/// `shipped`（扩展清单 `[[slash]]` 的 `access`），都没有的是 `admin`。
fn access(core: &Core, name: &str, shipped: Option<Access>) -> Access {
    CommandSettings::at(&core.config().resolved().values(), &[name])
        .access
        .as_deref()
        .and_then(Access::parse)
        .or(shipped)
        .unwrap_or(Access::Admin)
}

/// 照 `access` 判 `by` 过不过身份那一关：`admin` 照旧（[`may_run`]），`anyone` 都过，`off` 都不过。
fn passes(access: Access, by: &By) -> bool {
    match access {
        Access::Admin => may_run(by),
        Access::Anyone => true,
        Access::Off => false,
    }
}

/// 停用的命令 `name` 被拒：`command_not_allowed`，`data.why` 说「/<名字> 已停用。」（施工 O-6 六补）。
async fn turned_off(core: &Core, peer: &Peer, name: &str) -> Refusal {
    let said = Said {
        key: "commands/unavailable/off",
        fields: vec![("name", name.to_string())],
        lists: Vec::new(),
    };
    Refusal::COMMAND_NOT_ALLOWED.because(words(core, peer, &said).await)
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
    match loaded {
        Ok(Ok(human)) => sentence(&human, said),
        _ => {
            tracing::warn!(target: "miyu::endpoint", "command receipt words not read");
            String::new()
        }
    }
}

/// 一句：先把几段照这门语言拼好（每段照 `commands/joiner` 连起来），再和别的字一起填。缺字的是空的。
pub(crate) fn sentence(human: &Human, said: &Said) -> String {
    let joiner = Words::sentence(human, "commands/joiner", &[]).unwrap_or_default();
    let lists: Vec<(&str, String)> = said
        .lists
        .iter()
        .map(|(name, parts)| {
            let parts: Vec<String> = parts.iter().map(|part| sentence(human, part)).collect();
            (*name, parts.join(&joiner))
        })
        .collect();
    let fields: Vec<(&str, &str)> = said
        .fields
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .chain(lists.iter().map(|(name, value)| (*name, value.as_str())))
        .collect();
    Words::sentence(human, said.key, &fields).unwrap_or_default()
}

mod basic;
mod catalog;
mod declared;
mod goal;
mod looping;
mod pages;
mod prompts;
mod registry;
mod templates;
#[cfg(test)]
mod tests;
mod todo;
mod why;
mod workspace;

pub(crate) use catalog::catalog;
pub(crate) use declared::{Declared, found as declared_commands};
pub(crate) use templates::found as template_commands;
