//! 人生会话（施工 LF-5 中，设计 `70-推演人生.md` 第三节第 1、6、7 条，`docs/blueprint/life.md`）：`life.start`、`life.status`、
//! `life.pause`、`life.resume`，参数都是 `{persona}`。
//!
//! - 身份记在会话的日志里（`life.started`），真相只有日志；定时表里人生那一行（`loop` 是 `life`）只管下次几点醒。
//! - 「这个人格的人生会话是哪个」照会话索引找（`life` 那一格），索引落后的照日志补，和 `session.list` 同一个读法。
//! - 改的三个方法在 [`crate::Core`] 的一把锁里排着来：两个头同时开同一个人格的，开不出两条。
//! - `life.start` 四步：造会话、设上下文上限、记身份、建那一行。前两步没成的删掉刚造的会话；身份记上了、那一行没建成的，说清
//!   是哪个会话，`life.resume` 照日志重建那一行。
//! - 结束（施工 LF-4 上，`finish.rs`）：`life.kill`、`life.new`，她自己的 `end_life` 经 `port.rs`（施工 LF-4 下）。找的时候结束了的也算（最新的那一个）：`life.status` 说 `ended`，
//!   `life.pause`、`life.resume` 拒 `life_ended`，`life.start`、`life.new` 先把它没清完的清完再开新的。

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::LifeStarted;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::session::{Appended, Command, Outcome};
use miyu_session::Handle;
use miyu_store::index::{Life, Row};
use miyu_tool::Stop;

use crate::Core;
use crate::list::{LOCAL, scan};
use crate::methods::params;
use crate::refusal::Refusal;
use crate::schedules::methods::refusal as schedule_refusal;
use crate::schedules::{Creator, Loop, Schedule, Target, Wanted, When};
use crate::sessions::{Opening, person};
use crate::wire::Request;

mod finish;
mod port;

pub(crate) use finish::finish;
pub(crate) use port::Ending;

/// 出厂的人生会话用哪个预设（`resources/presets/life.toml`）。
const PRESET: &str = "life";

/// 新开的人生会话的上下文上限（token，设计 70 第四节）：她隔一两个小时才醒一次，供应商的缓存多半已经失效，每次醒来整份上下文
/// 按原价算，不设的话一周后每次醒来要带二十多万 token（2026-11-14 项目主人定限制上下文长度）。主人以后照常能改（设计 68
/// 的 `context`）。同 `LIFE_GAP` 是写在这里的数，不是配置项；照这一刻的模型最少要的比它大的，照最少要的设。
const CONTEXT: u64 = 32_000;

/// 四个方法的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifeParams {
    persona: String,
}

/// 分给六个方法：先查人格在不在。
pub(crate) async fn call(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let LifeParams { persona } = params(request)?;
    crate::personas::resolve(core, Some(Some(&persona))).await?;
    if request.method == "life.status" {
        let found = find(core, &persona).await?;
        return state(core, found.as_ref());
    }
    let _one_at_a_time = core.life.lock().await;
    let found = find(core, &persona).await?;
    match request.method.as_str() {
        "life.start" => {
            match &found {
                Some((session, Life::Started)) => return Err(Refusal::life_exists(session)),
                // 结束了的：先把没清完的清完（重做不重复清），再开新的。
                Some((session, _)) => end(core, session).await?,
                None => {}
            }
            start(core, request, &persona).await
        }
        "life.new" => {
            let ended = match &found {
                Some((session, life)) => {
                    end(core, session).await?;
                    (*life == Life::Started).then(|| session.as_str().to_string())
                }
                None => None,
            };
            let mut reply = start(core, request, &persona).await?;
            if let Some(ended) = ended {
                reply["ended"] = json!(ended);
            }
            Ok(reply)
        }
        "life.kill" => {
            let (session, _) = found.ok_or(Refusal::NO_LIFE)?;
            end(core, &session).await?;
            Ok(json!({"state": "ended", "session": session.as_str()}))
        }
        "life.pause" => {
            let session = running(found)?;
            if let Some(row) = row_of(core, &session)?
                && !row.paused
            {
                core.schedules().pause(&row.id).map_err(schedule_refusal)?;
            }
            state(core, Some(&(session, Life::Started)))
        }
        "life.resume" => {
            let session = running(found)?;
            match row_of(core, &session)? {
                Some(row) if row.paused => {
                    core.schedules().resume(&row.id).map_err(schedule_refusal)?;
                }
                Some(_) => {}
                // 表坏了换过空的、那一行没建成的：照日志里的身份重建。
                None => {
                    schedule(core, &session)?;
                }
            }
            state(core, Some(&(session, Life::Started)))
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// 没结束的那一条：没有的 `no_life`，结束了的 `life_ended`（带那个会话）。
fn running(found: Option<(SessionId, Life)>) -> Result<SessionId, Refusal> {
    match found {
        None => Err(Refusal::NO_LIFE),
        Some((session, Life::Started)) => Ok(session),
        Some((session, _)) => Err(Refusal::life_ended(&session)),
    }
}

/// 人结束会话 `session` 上的人生，等清完：已经结束了的照重做。
async fn end(core: &Arc<Core>, session: &SessionId) -> Result<(), Refusal> {
    let finishing = finish(core, session, person(), None).await?;
    finishing
        .done()
        .await
        .map_err(|refusal| refusal.about_session(session))
}

/// `life.start`：见模块的说明。回应 `{"session"}`。
async fn start(core: &Arc<Core>, request: &Request, persona: &str) -> Result<Value, Refusal> {
    let who = Opening {
        attended: false,
        oneshot: false,
        model: None,
        venue: None,
        memory: None,
        preset: Some(PRESET.to_string()),
        group: None,
        profile: false,
        images: true,
        life: true,
        titles: true,
        origin: None,
        confined: false,
    };
    let created = core
        .sessions
        .create(
            core,
            request.id.clone(),
            Some(Some(persona)),
            "~".to_string(),
            Vec::new(),
            who,
        )
        .await?;
    let session = created.id;
    core.schedules().named(&session, persona);
    if let Err(refusal) = mark(core, request, &session).await {
        // 还不是人生会话的空会话：删掉，不留半截。
        if let Err(left) = core.sessions.delete(core, &session).await {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), reason = left.reason, "life session not removed");
        }
        return Err(refusal);
    }
    schedule(core, &session).map_err(|refusal| refusal.about_session(&session))?;
    Ok(json!({"session": session.as_str()}))
}

/// 新开的会话设上下文上限、记身份（命令编号接在请求的编号后面：`/context`、`/life`）。
async fn mark(core: &Arc<Core>, request: &Request, session: &SessionId) -> Result<(), Refusal> {
    let found = core.sessions.get(core, session).await?;
    let least = found
        .handle
        .current()
        .await
        .map_err(|_| Refusal::STOPPED)?
        .least_context
        .unwrap_or(1);
    let context = crate::models::context::command(&found.handle, Some(CONTEXT.max(least))).await?;
    send(core, request, session, &found.handle, "context", context).await?;
    let identity = Command::Append {
        event: Appended::Life(LifeStarted {}),
    };
    send(core, request, session, &found.handle, "life", identity).await
}

/// 照这条请求的编号接 `/<suffix>` 给会话发一个命令，记成人发的。拒绝的照内核的原因码；会话停了的从表里拿掉。
async fn send(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    suffix: &str,
    command: Command,
) -> Result<(), Refusal> {
    let id = CommandId::parse(&format!("{}/{suffix}", request.id.as_str()))
        .map_err(|_| Refusal::BAD_PARAMS)?;
    match handle.command(id, person(), command).await {
        Ok(Outcome::Rejected { reason, about }) => Err(Refusal::kernel_about(reason, &about)),
        Ok(_) => Ok(()),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 建人生那一行：跑在 `session` 上、人建的、留言是空的、第一次是现在（马上醒）。
fn schedule(core: &Core, session: &SessionId) -> Result<Schedule, Refusal> {
    let now = core.schedules().now();
    let wanted = Wanted {
        by: Creator::Person,
        when: When::At { at: now },
        prompt: String::new(),
        target: Target::Session {
            session: session.clone(),
        },
        looping: Some(Loop::Life),
        first: Some(now),
    };
    core.schedules().add(wanted).map_err(schedule_refusal)
}

/// 人格 `persona` 的人生会话：本机的主会话里，照会话索引（落后的照日志补）认 `life`、这一刻的人格，最新的那一个，连同它开着
/// 还是结束了（施工 LF-4 上）。只照不会变的格（父会话、场所）先挑：`life`、人格会在造好以后才变，落后的那一行照它挑会漏掉。
async fn find(core: &Arc<Core>, persona: &str) -> Result<Option<(SessionId, Life)>, Refusal> {
    let root = core.root.clone();
    let index = core.index();
    let found = tokio::task::spawn_blocking(move || {
        let main = |row: &Row| row.parent.is_none() && row.venue == LOCAL;
        scan(
            &root,
            Some(&*index),
            &BTreeSet::new(),
            main,
            None,
            &Stop::default(),
        )
    })
    .await;
    match found {
        Ok(Ok(listed)) => Ok(listed
            .into_iter()
            .find(|one| one.life != Life::Not && one.persona.as_deref() == Some(persona))
            .map(|one| (one.id, one.life))),
        Ok(Err(error)) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "life session not found");
            Err(Refusal::INTERNAL)
        }
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// 定时表里跑在 `session` 上的人生那一行；没有的是空的。
fn row_of(core: &Core, session: &SessionId) -> Result<Option<Schedule>, Refusal> {
    Ok(core
        .schedules()
        .list()
        .map_err(schedule_refusal)?
        .into_iter()
        .find(|one| {
            one.looping == Some(Loop::Life)
                && matches!(&one.target, Target::Session { session: on } if on == session)
        }))
}

/// `life.status` 的回应（`life.pause`、`life.resume` 同）：没有人生会话的 `none`；结束了的 `ended`，带会话（施工 LF-4 上）；那一行
/// 在、没停的 `running`，带下一次；停着的、那一行不在的 `paused`。
fn state(core: &Core, found: Option<&(SessionId, Life)>) -> Result<Value, Refusal> {
    let Some((session, life)) = found else {
        return Ok(json!({"state": "none"}));
    };
    if *life == Life::Ended {
        return Ok(json!({"state": "ended", "session": session.as_str()}));
    }
    let mut reply = json!({"state": "paused", "session": session.as_str()});
    if let Some(row) = row_of(core, session)?
        && !row.paused
    {
        reply["state"] = json!("running");
        if let Some(next) = row.next {
            reply["next"] = json!(next);
        }
    }
    crate::schedules::doing_shown(core, session, &mut reply);
    Ok(reply)
}
