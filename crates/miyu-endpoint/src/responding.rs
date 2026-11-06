//! `session.respond {session, to, facts}`（施工 O-14 上，`docs/blueprint/chat.md` 第七条第 3 条第 1 项）：照已经旁听记下的几条开
//! 一轮，回应 `{"events": [...]}`（同 `session.send`）。`to` 是最多 64 条序号，内核照序号排好、去重；`facts` 可以不写，每块
//! `{kind, text}`，`kind` 照事实类别的写法，`text` 最多 4 KiB，原样记成 `context.injected`。`to` 不写或是空的照 `facts` 开
//! （施工 O-64），这时 `facts` 至少一块。写错的 `bad_params`，什么都不记。
//! `tail` 可以不写（施工 RT-1，项目主人 11-07 要的，im 在群聊每一轮末尾带一句固定的提示）：字符串，最多 2 KiB，空的当没写；
//! 开出来的这一轮每次请求都把它接在最末尾，不进历史；不是字符串、超长的 `bad_params`，`data.field` 是 `tail`。
//! 记成谁同 `events.append`：核心拉起的扩展是那个包（模块），别的是主人。内核拒的照原因码回，`not_ambient`、
//! `already_answered` 的 `data.messages` 是不合的那几条。
//!
//! `session.note {session, facts}`（施工 O-14 补，`docs/blueprint/venues.md`「记几块事实」）：只记事实、不开回合，回应同上。
//! `facts` 1 到 16 块，写法同上。正在跑一轮的带这一轮的回合编号，空闲的下一轮开头看到。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::ContextInjected;
use miyu_kernel::id::{FactKind, Seq, SessionId};
use miyu_kernel::session::Command;

use crate::Core;
use crate::appending::recorder;
use crate::hello::Caller;
use crate::methods::command_by;
use crate::refusal::Refusal;
use crate::wire::Request;

/// `to` 最多几条。
const TRIGGERS: usize = 64;
/// 一块事实的 `text` 最多几个字节。
const FACT: usize = 4 * 1024;
/// `session.note` 一次最多几块。
const NOTED: usize = 16;
/// `tail` 最多几个字节（施工 RT-1）。
const TAIL: usize = 2 * 1024;

/// `session.respond` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RespondParams {
    session: String,
    #[serde(default)]
    to: Vec<u64>,
    #[serde(default)]
    facts: Vec<FactParams>,
    /// 先收成任意的值，自己查：写错了要报 `data.field`（施工 RT-1）。
    #[serde(default)]
    tail: Option<Value>,
}

/// `session.note` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoteParams {
    session: String,
    facts: Vec<FactParams>,
}

/// 一块事实。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FactParams {
    kind: String,
    text: String,
}

/// `session.respond`：见模块的说明。
pub(crate) async fn respond(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
    params: RespondParams,
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    if (params.to.is_empty() && params.facts.is_empty()) || params.to.len() > TRIGGERS {
        return Err(Refusal::BAD_PARAMS);
    }
    let to = params
        .to
        .iter()
        .map(|&seq| Seq::new(seq).ok_or(Refusal::BAD_PARAMS))
        .collect::<Result<Vec<_>, _>>()?;
    let facts = facts(params.facts)?;
    let tail = tail(params.tail)?;
    let found = core.sessions.get(core, &session).await?;
    let by = recorder(caller)?;
    let command = Command::Respond { to, facts, tail };
    let events = command_by(core, request, &session, &found.handle, by, command).await?;
    Ok(json!({ "events": events }))
}

/// `session.note`：见模块的说明。
pub(crate) async fn note(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
    params: NoteParams,
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    if params.facts.is_empty() || params.facts.len() > NOTED {
        return Err(Refusal::BAD_PARAMS);
    }
    let facts = facts(params.facts)?;
    let found = core.sessions.get(core, &session).await?;
    let by = recorder(caller)?;
    let command = Command::Note { facts };
    let events = command_by(core, request, &session, &found.handle, by, command).await?;
    Ok(json!({ "events": events }))
}

/// 查过写法的事实：类别照写法，`text` 最多 [`FACT`] 字节，有一块不对就 `bad_params`。
fn facts(facts: Vec<FactParams>) -> Result<Vec<ContextInjected>, Refusal> {
    facts
        .into_iter()
        .map(|fact| {
            let kind = FactKind::parse(&fact.kind).map_err(|_| Refusal::BAD_PARAMS)?;
            if fact.text.len() > FACT {
                return Err(Refusal::BAD_PARAMS);
            }
            Ok(ContextInjected {
                kind,
                text: fact.text,
                refs: Vec::new(),
                after: false,
            })
        })
        .collect()
}

/// `tail` 查过的样子（施工 RT-1）：没写的、`null`、空串都是没有；不是字符串、超过 [`TAIL`] 的 `bad_params`，`data.field` 是 `tail`。
pub(crate) fn tail(value: Option<Value>) -> Result<Option<String>, Refusal> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.is_empty() => Ok(None),
        Some(Value::String(text)) if text.len() <= TAIL => Ok(Some(text)),
        Some(_) => Err(Refusal::bad_field("tail")),
    }
}
