//! `/life`（施工 LF-5 下，设计 `70-推演人生.md` 第二节第 11 条，`docs/blueprint/life.md`「命令」）：人在本机的会话里看、开、停、
//! 续、结束这个会话的人格的人生会话，不用先找到那个会话。命令只是 `life.*` 的另一个入口：照这个会话这一刻的人格调
//! [`crate::life::act`]，这里只认参数、拼回执，回应多带一格 `session`（人生会话的编号，有才带），头照它跳过去。
//!
//! - `/life`：有在跑、停着的回它的状态；没有的、上一条结束了的**不开**，只回说明（这是什么、会自己醒、每次醒是一次请求、
//!   会多花 token、会自己记事、怎么开）。
//! - `/life new`、`/life pause`、`/life resume`、`/life kill` 同那几个方法；没有人生会话的（结束了的也算没有）回没有。
//! - 不做确认：打了就做（2026-11-14 项目主人定）。
//!
//! 谁能用同 `/workspace`：只给主人本人；场所会话里照没有这条命令。核心自带，不跟包、不跟预设。认不出的参数 `bad_params`，
//! `data.why` 是用法那一句。
//!
//! 施工 LF-12（项目主人 2026-11-17 本机验收第 25、29 条）：
//!
//! - 不带人格的会话里照所有人格的找（`life/bare.rs`）：只有一个人格有的照它办，回执写上是哪个人格的。
//! - 回应带 `session` 的都多带一格 `persona`（它是哪个人格的）。
//! - 光打的 `/life` 多带一格 `lives`：所有人格没结束的人生会话（[`crate::life::lives`]），这个会话的人格的排最前；头照它画
//!   「快速跳转人生会话」。一条都没有的不写。

use std::sync::Arc;

use serde_json::Value;

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_session::Handle;

use super::registry::{Call, Carried, Commands, Jump, Ran, Running, Slash};
use super::{Said, words};
use crate::Core;
use crate::life::{Act, Heaped, act};
use crate::list::LOCAL;
use crate::refusal::Refusal;

mod bare;
mod here;

pub(super) use here::{living, looped, stopped};

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
                return bare::run(&call, asked).await;
            };
            crate::personas::resolve(call.core, Some(Some(&persona))).await?;
            let mut done = run(call.core, call.id, &persona, asked, None).await?;
            if matches!(asked, Asked::Show) {
                let lives = crate::life::lives(call.core, call.peer.language).await?;
                listing(&mut done, lives, Some(&persona));
            }
            Ok(done)
        })
    }
}

/// 光打的 `/life` 的回应带上所有人格没结束的人生会话 `lives`（施工 LF-12）：这个会话的人格 `own` 的排最前，其余照原来的
/// 先后（人格的编号）。一条都没有的不带。
fn listing(done: &mut Ran, mut lives: Vec<Value>, own: Option<&str>) {
    if lives.is_empty() {
        return;
    }
    if let Some(own) = own {
        lives.sort_by_key(|one| one["persona"] != own);
    }
    done.life.get_or_insert_with(Carried::default).lives = lives;
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

/// 状态、结束的回执各两个键：带人格的会话里说的就是这个会话的人格，不写名字；不带人格的会话里（施工 LF-12）写上是谁的。
const RUNNING: Keys = ("commands/life-running", "commands/life-running-of");
const AWAKE: Keys = ("commands/life-awake", "commands/life-awake-of");
const PAUSED: Keys = ("commands/life-paused", "commands/life-paused-of");
const ENDED: Keys = ("commands/life-ended", "commands/life-ended-of");

/// 一句回执的两个键：不写名字的、写名字的（多填一格 `persona`）。
type Keys = (&'static str, &'static str);

/// 照写不写名字挑键：`name` 是不带人格的会话里找到的那个人格给人看的名字，带人格的会话里是空的。
fn say(keys: Keys, name: Option<&str>, mut fields: Vec<(&'static str, String)>) -> Said {
    match name {
        Some(name) => {
            fields.push(("persona", name.to_string()));
            Said::with(keys.1, fields)
        }
        None => Said::with(keys.0, fields),
    }
}

/// 照人格 `persona` 做 `asked`，拼回执；`name` 见 [`say`]。
fn run<'a>(
    core: &'a Arc<Core>,
    id: &'a CommandId,
    persona: &'a str,
    asked: Asked,
    name: Option<&'a str>,
) -> Heaped<'a, Result<Ran, Refusal>> {
    Box::pin(async move {
        let who = (persona, name);
        match asked {
            Asked::Show => {
                let now = act(core, id, persona, Act::Status).await?;
                shown(core, &now, who).await
            }
            Asked::New => {
                let opened = act(core, id, persona, Act::New).await?;
                let key = match opened.get("ended") {
                    Some(_) => "commands/life-renewed",
                    None => "commands/life-started",
                };
                Ok(ran(Said::plain(key), jump(&opened, persona)))
            }
            Asked::Switch(switch) => match act(core, id, persona, switch).await {
                Ok(now) => shown(core, &now, who).await,
                Err(refusal) if none(&refusal) => Ok(nothing()),
                Err(refusal) => Err(refusal),
            },
            Asked::Kill => {
                // 结束了的也算没有：`life.kill` 对它是把清理再走一遍，这里不说「已结束」。
                let now = act(core, id, persona, Act::Status).await?;
                if !matches!(state(&now), "running" | "paused") {
                    return Ok(nothing());
                }
                match act(core, id, persona, Act::Kill).await {
                    Ok(ended) => Ok(ran(say(ENDED, name, Vec::new()), jump(&ended, persona))),
                    Err(refusal) if none(&refusal) => Ok(nothing()),
                    Err(refusal) => Err(refusal),
                }
            }
        }
    })
}

/// 照 `life.status` 那个样子的回应说状态：没有的、结束了的回说明、不带会话；别的照 [`told`]，带它。`who` 是人格的编号和
/// 要不要写的名字（[`say`]）。
async fn shown(
    core: &Arc<Core>,
    now: &Value,
    (persona, name): (&str, Option<&str>),
) -> Result<Ran, Refusal> {
    match told(core, now, name).await {
        Some(said) => Ok(ran(said, jump(now, persona))),
        None => Ok(ran(Said::plain("commands/life-about"), None)),
    }
}

/// 没结束的人生会话的状态那一句：停着的说停着；在跑的她这会儿醒着就说醒着，不然说下次几点醒（核心这台机器的时区，到
/// 分钟）。没有的、结束了的没有这一句。
fn told<'a>(
    core: &'a Arc<Core>,
    now: &'a Value,
    name: Option<&'a str>,
) -> Heaped<'a, Option<Said>> {
    Box::pin(async move {
        let session = now["session"].as_str()?;
        match state(now) {
            "running" => {
                let next = now["next"]
                    .as_str()
                    .and_then(|next| Timestamp::parse(next).ok());
                Some(match (awake(core, session).await, next) {
                    (false, Some(next)) => {
                        let next = next.local_minute(crate::sessions::offset());
                        say(RUNNING, name, vec![("next", next)])
                    }
                    _ => say(AWAKE, name, Vec::new()),
                })
            }
            "paused" => Some(say(PAUSED, name, Vec::new())),
            _ => None,
        }
    })
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

/// 回应里的 `state`。
fn state(reply: &Value) -> &str {
    reply["state"].as_str().unwrap_or_default()
}

/// 回应里的 `session`，连同它是人格 `persona` 的：头要跳过去的那一条。
fn jump(reply: &Value, persona: &str) -> Option<Jump> {
    reply["session"].as_str().map(|session| Jump {
        session: session.to_string(),
        persona: persona.to_string(),
    })
}

/// 没有可停、可续、可结束的：`no_life`，或者最近那一条已经结束（`life_ended`）。
fn none(refusal: &Refusal) -> bool {
    refusal.reason == Refusal::NO_LIFE.reason || refusal.reason == "life_ended"
}

/// 「这个人格没有人生会话。」
fn nothing() -> Ran {
    ran(Said::plain("commands/life-nothing"), None)
}

/// 什么都不追加，回执 `said`，带头要跳过去的人生会话 `jump`。
fn ran(said: Said, jump: Option<Jump>) -> Ran {
    Ran {
        events: Vec::new(),
        said,
        life: jump.map(|jump| Carried {
            jump: Some(jump),
            lives: Vec::new(),
        }),
    }
}
