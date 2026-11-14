//! `/life`（施工 LF-5 下，设计 `70-推演人生.md` 第二节第 11 条，`docs/blueprint/life.md`「命令」）：人在本机的会话里看、开、停、
//! 续、结束这个会话的人格的人生会话，不用先找到那个会话。命令只是 `life.*` 的另一个入口：照这个会话这一刻的人格调
//! [`crate::life::act`]，这里只认参数、拼回执，回应多带一格 `session`（人生会话的编号，有才带），头照它跳过去。
//!
//! - `/life`：有在跑、停着的回它的状态；没有的、上一条结束了的**不开**，只回说明（这是什么、会自己醒、每次醒是一次请求、
//!   会多花 token、会自己记事、怎么开）。
//! - `/life new`、`/life pause`、`/life resume`、`/life kill` 同那几个方法；没有人生会话的（结束了的也算没有）回没有。
//! - 不做确认：打了就做（2026-11-14 项目主人定）。
//!
//! 谁能用同 `/workspace`：只给主人本人；场所会话里照没有这条命令。核心自带，不跟包、不跟预设。不带人格的会话回一句没有人格；
//! 认不出的参数 `bad_params`，`data.why` 是用法那一句。

use std::sync::Arc;

use serde_json::Value;

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_session::Handle;

use super::registry::{Call, Commands, Ran, Running, Slash};
use super::{Said, words};
use crate::Core;
use crate::life::{Act, act};
use crate::list::LOCAL;
use crate::refusal::Refusal;

/// 登记 `/life`。
pub(super) fn register(commands: &mut Commands) {
    commands.register(Arc::new(LifeCommand));
}

/// `/life`。
struct LifeCommand;

impl Slash for LifeCommand {
    fn name(&self) -> &'static str {
        "life"
    }

    fn summary(&self) -> &'static str {
        "commands/summary/life"
    }

    fn argument(&self) -> Option<&'static str> {
        Some("commands/argument/life")
    }

    fn usage(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("", "commands/usage/life/show"),
            ("new", "commands/usage/life/new"),
            ("pause", "commands/usage/life/pause"),
            ("resume", "commands/usage/life/resume"),
            ("kill", "commands/usage/life/kill"),
        ]
    }

    /// 场所会话里照没有这条命令；开、结束的是主人的人格的会话：管理的人不行。
    fn allowed(&self, _core: &Core, handle: &Handle, by: &By) -> Result<(), Refusal> {
        if handle.venue().as_str() != LOCAL {
            return Err(Refusal::UNKNOWN_COMMAND);
        }
        match by.is_owner() {
            true => Ok(()),
            false => Err(Refusal::OWNER_ONLY),
        }
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let Some(asked) = parse(call.rest) else {
                let usage = words(call.core, call.peer, &Said::plain("commands/life-usage")).await;
                return Err(Refusal::BAD_PARAMS.because(usage));
            };
            let persona = call
                .handle()
                .current()
                .await
                .map_err(|_| Refusal::STOPPED)?
                .persona;
            let Some(persona) = persona else {
                return Ok(ran(Said::plain("commands/life-no-persona"), None));
            };
            crate::personas::resolve(call.core, Some(Some(&persona))).await?;
            run(call.core, call.id, &persona, asked).await
        })
    }
}

/// 人要做的。
#[derive(Debug, Clone, Copy)]
enum Asked {
    /// `/life`。
    Show,
    /// `/life new`。
    New,
    /// `/life pause`、`/life resume`。
    Switch(Act),
    /// `/life kill`。
    Kill,
}

/// 读 `/life` 后面的字（去掉了前后空白）：认不出的没有。
fn parse(rest: &str) -> Option<Asked> {
    Some(match rest {
        "" => Asked::Show,
        "new" => Asked::New,
        "pause" => Asked::Switch(Act::Pause),
        "resume" => Asked::Switch(Act::Resume),
        "kill" => Asked::Kill,
        _ => return None,
    })
}

/// 照人格 `persona` 做 `asked`，拼回执。
async fn run(
    core: &Arc<Core>,
    id: &CommandId,
    persona: &str,
    asked: Asked,
) -> Result<Ran, Refusal> {
    match asked {
        Asked::Show => {
            let now = boxed(core, id, persona, Act::Status).await?;
            shown(core, &now).await
        }
        Asked::New => {
            let opened = boxed(core, id, persona, Act::New).await?;
            let key = match opened.get("ended") {
                Some(_) => "commands/life-renewed",
                None => "commands/life-started",
            };
            Ok(ran(Said::plain(key), session_of(&opened)))
        }
        Asked::Switch(switch) => match boxed(core, id, persona, switch).await {
            Ok(now) => shown(core, &now).await,
            Err(refusal) if none(&refusal) => Ok(nothing()),
            Err(refusal) => Err(refusal),
        },
        Asked::Kill => {
            // 结束了的也算没有：`life.kill` 对它是把清理再走一遍，这里不说「已结束」。
            let now = boxed(core, id, persona, Act::Status).await?;
            if !matches!(state(&now), "running" | "paused") {
                return Ok(nothing());
            }
            match boxed(core, id, persona, Act::Kill).await {
                Ok(ended) => Ok(ran(Said::plain("commands/life-ended"), session_of(&ended))),
                Err(refusal) if none(&refusal) => Ok(nothing()),
                Err(refusal) => Err(refusal),
            }
        }
    }
}

/// 照 `life.status` 那个样子的回应说状态：没有的、结束了的回说明、不带会话；停着的说停着；在跑的她这会儿醒着就说醒着，
/// 不然说下次几点醒（核心这台机器的时区，到分钟）。
async fn shown(core: &Arc<Core>, now: &Value) -> Result<Ran, Refusal> {
    let session = session_of(now);
    let said = match (state(now), &session) {
        ("running", Some(id)) => {
            let next = now["next"]
                .as_str()
                .and_then(|next| Timestamp::parse(next).ok());
            match (awake(core, id).await, next) {
                (false, Some(next)) => Said::with(
                    "commands/life-running",
                    vec![("next", next.local_minute(crate::sessions::offset()))],
                ),
                _ => Said::plain("commands/life-awake"),
            }
        }
        ("paused", Some(_)) => Said::plain("commands/life-paused"),
        _ => return Ok(ran(Said::plain("commands/life-about"), None)),
    };
    Ok(ran(said, session))
}

/// 人生会话 `id` 这会儿在跑一轮（她醒着）。找不到的当没醒。
async fn awake(core: &Arc<Core>, id: &str) -> bool {
    let Ok(session) = SessionId::parse(id) else {
        return false;
    };
    match core.sessions.get(core, &session).await {
        Ok(found) => found.handle.busy(),
        Err(_) => false,
    }
}

/// 同 [`act`]，放在堆上：开、结束的那一串很深，整个塞进 `command.run` 的那一个 future 里，调试版的测试线程栈不够。
async fn boxed(
    core: &Arc<Core>,
    id: &CommandId,
    persona: &str,
    what: Act,
) -> Result<Value, Refusal> {
    Box::pin(act(core, id, persona, what)).await
}

/// 回应里的 `state`。
fn state(reply: &Value) -> &str {
    reply["state"].as_str().unwrap_or_default()
}

/// 回应里的 `session`。
fn session_of(reply: &Value) -> Option<String> {
    reply["session"].as_str().map(str::to_string)
}

/// 没有可停、可续、可结束的：`no_life`，或者最近那一条已经结束（`life_ended`）。
fn none(refusal: &Refusal) -> bool {
    refusal.reason == Refusal::NO_LIFE.reason || refusal.reason == "life_ended"
}

/// 「这个人格没有人生会话。」
fn nothing() -> Ran {
    ran(Said::plain("commands/life-nothing"), None)
}

/// 什么都不追加，回执 `said`，带人生会话 `session`。
fn ran(said: Said, session: Option<String>) -> Ran {
    Ran {
        events: Vec::new(),
        said,
        session,
    }
}
