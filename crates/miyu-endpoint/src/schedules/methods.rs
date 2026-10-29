//! `schedule.*`（施工 J-1 下，`docs/blueprint/schedules.md` 第五节，设计 `45-定时和循环.md` 第三节第 3 条）：人经协议建、列、
//! 删、停、续定时任务。一项的写法在 [`entry`]，推送
//! （`schedules.changed`）也用它。
//!
//! 施工 J-4：扩展也能调（要批过 `sessions.drive`），建的记成包建的；它只看得到、改得了自己建的，别的编号一样 `unknown_schedule`。
//! 只有扩展能写「跑在哪」是 `package`（到点推 `schedule.fired` 给它自己，`fire.rs`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;

use super::{Creator, Loop, Schedule, ScheduleError, Target, Wanted, When};
use crate::Core;
use crate::hello::Caller;
use crate::methods::params;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 分给几个方法。
pub(crate) async fn call(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let package = caller.package.as_deref();
    let schedules = core.schedules();
    match request.method.as_str() {
        "schedule.add" => {
            let wanted = wanted(core, package, params(request)?).await?;
            let added = schedules.add(wanted).map_err(refusal)?;
            Ok(json!({"schedule": entry(&added)}))
        }
        "schedule.list" => {
            let ListParams { session } = params(request)?;
            let session = session
                .map(|text| SessionId::parse(&text).map_err(|_| Refusal::BAD_PARAMS))
                .transpose()?;
            let listed: Vec<Value> = schedules
                .list()
                .map_err(refusal)?
                .iter()
                .filter(|schedule| mine(package, schedule))
                .filter(|schedule| {
                    session.as_ref().is_none_or(|session| {
                        matches!(&schedule.target, Target::Session { session: on } if on == session)
                    })
                })
                .map(entry)
                .collect();
            Ok(json!({"schedules": listed}))
        }
        "schedule.cancel" => {
            let IdParams { id } = params(request)?;
            own(core, package, &id)?;
            schedules.cancel(&id).map_err(refusal)?;
            Ok(json!({}))
        }
        "schedule.pause" => {
            let IdParams { id } = params(request)?;
            own(core, package, &id)?;
            let paused = schedules.pause(&id).map_err(refusal)?;
            Ok(json!({"schedule": entry(&paused)}))
        }
        "schedule.resume" => {
            let IdParams { id } = params(request)?;
            own(core, package, &id)?;
            let resumed = schedules.resume(&id).map_err(refusal)?;
            Ok(json!({"schedule": entry(&resumed)}))
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// 这个连接看得到的：人看得到全部，扩展只看得到包 `package` 建的（施工 J-4）。
fn mine(package: Option<&str>, schedule: &Schedule) -> bool {
    match (package, &schedule.by) {
        (None, _) => true,
        (Some(package), Creator::Package { package: by }) => by == package,
        (Some(_), _) => false,
    }
}

/// 扩展只改得了自己建的（施工 J-4）：别的、没有的都是 `unknown_schedule`。人的不查。
fn own(core: &Core, package: Option<&str>, id: &str) -> Result<(), Refusal> {
    if package.is_none() {
        return Ok(());
    }
    let schedule = core.schedules().get(id).map_err(refusal)?;
    if mine(package, &schedule) {
        Ok(())
    } else {
        Err(Refusal::UNKNOWN_SCHEDULE)
    }
}

/// 订阅 `schedules` 的回应：这一刻的全部。
pub(crate) fn listed(core: &Core) -> Result<Value, Refusal> {
    let listed: Vec<Value> = core
        .schedules()
        .list()
        .map_err(refusal)?
        .iter()
        .map(entry)
        .collect();
    Ok(json!({"schedules": listed}))
}

/// 推送里的一项：任务 `id` 这一刻的样子，没有了的是删了；表读不成的没有（不推）。
pub(crate) fn changed(core: &Core, id: &str) -> Option<Value> {
    match core.schedules().get(id) {
        Ok(schedule) => Some(json!({"id": id, "entry": entry(&schedule)})),
        Err(ScheduleError::Unknown) => Some(json!({"id": id, "removed": true})),
        Err(_) => None,
    }
}

/// `schedule.add` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddParams {
    when: WhenParams,
    prompt: String,
    #[serde(default)]
    target: Option<TargetParams>,
}

/// 什么时候：`cron`（可带 `recurring`）、`at`、`every` 三种写一种。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WhenParams {
    cron: Option<String>,
    recurring: Option<bool>,
    /// cron 照哪个时区读（施工 J-3 上）：IANA 的名字；不写的记这台机器的。只能和 `cron` 一起写。
    zone: Option<String>,
    at: Option<String>,
    every: Option<u64>,
}

/// 跑在哪：`session`、`new`、`package` 写一种；都不写的是每次新开。`package` 只有扩展能写（施工 J-4）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetParams {
    session: Option<String>,
    new: Option<NewParams>,
    package: Option<PackageParams>,
}

/// 推给建它的扩展自己：没有可写的。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageParams {}

/// 每次新开的：人格（`null` 是无人格）、预设、工作目录，不写的照这时的默认定下来。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct NewParams {
    #[serde(default, deserialize_with = "crate::personas::written")]
    persona: Option<Option<String>>,
    preset: Option<String>,
    cwd: Option<String>,
}

/// `schedule.list` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListParams {
    #[serde(default)]
    session: Option<String>,
}

/// 只带编号的几个方法的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdParams {
    id: String,
}

/// 照参数拼出要建的：时刻、会话都查过；每次新开的人格、预设照这时的默认定下来。扩展（`package`）建的记成包建的。
async fn wanted(
    core: &Arc<Core>,
    package: Option<&str>,
    params: AddParams,
) -> Result<Wanted, Refusal> {
    let when = match params.when {
        WhenParams {
            cron: Some(cron),
            recurring,
            zone,
            at: None,
            every: None,
        } => When::Cron {
            cron,
            recurring: recurring.unwrap_or(true),
            zone,
        },
        WhenParams {
            cron: None,
            recurring: None,
            zone: None,
            at: Some(at),
            every: None,
        } => {
            let at = Timestamp::parse(&at).map_err(|_| Refusal::bad_schedule("when"))?;
            if at <= core.schedules().now() {
                return Err(Refusal::bad_schedule("past"));
            }
            When::At { at }
        }
        WhenParams {
            cron: None,
            recurring: None,
            zone: None,
            at: None,
            every: Some(every),
        } => When::Every { every },
        _ => return Err(Refusal::bad_schedule("when")),
    };
    if params.prompt.trim().is_empty() {
        return Err(Refusal::bad_schedule("prompt"));
    }
    let target = match params.target {
        Some(TargetParams {
            session: Some(session),
            new: None,
            package: None,
        }) => Target::Session {
            session: session_of(core, &session).await?,
        },
        None
        | Some(TargetParams {
            session: None,
            new: None,
            package: None,
        }) => opened(core, NewParams::default()).await?,
        Some(TargetParams {
            session: None,
            new: Some(new),
            package: None,
        }) => opened(core, new).await?,
        Some(TargetParams {
            session: None,
            new: None,
            package: Some(PackageParams {}),
        }) if package.is_some() => Target::Package { missed: 0 },
        // 人写了 `package`：没有「建它的扩展」可推。
        Some(TargetParams {
            package: Some(_), ..
        }) if package.is_none() => return Err(Refusal::BAD_PARAMS),
        Some(_) => return Err(Refusal::bad_schedule("target")),
    };
    let by = match package {
        Some(package) => Creator::Package {
            package: package.to_string(),
        },
        None => Creator::Person,
    };
    Ok(Wanted {
        by,
        when,
        prompt: params.prompt,
        target,
        looping: None,
        first: None,
    })
}

/// 跑在已有的会话：要有。
async fn session_of(core: &Arc<Core>, text: &str) -> Result<SessionId, Refusal> {
    let session = SessionId::parse(text).map_err(|_| Refusal::bad_schedule("session"))?;
    match core.sessions.get(core, &session).await {
        Ok(_) => Ok(session),
        _ => Err(Refusal::bad_schedule("session")),
    }
}

/// 每次新开的：人格、预设照这时的默认定下来（找不到的照开会话的拒绝），工作目录不写的是家目录。
async fn opened(core: &Arc<Core>, new: NewParams) -> Result<Target, Refusal> {
    let persona = crate::personas::resolve(core, new.persona.as_ref().map(Option::as_deref))
        .await?
        .map(|found| found.id);
    let preset = crate::presets::resolve(core, new.preset.as_deref())
        .await?
        .id;
    // 写了的建时就查（施工 J-3 上）：到点开不成的每次只记一句 `not_opened`，人不知道。
    if let Some(cwd) = &new.cwd
        && !usable(core, cwd)
    {
        return Err(Refusal::bad_schedule("cwd"));
    }
    let cwd = new.cwd.unwrap_or_else(|| {
        let (_, own) = crate::sessions::roots(core);
        own.to_string_lossy().into_owned()
    });
    Ok(Target::New {
        persona,
        preset,
        cwd,
    })
}

/// 每次新开的工作目录用得了：照开会话的同一套换成真实位置（`~` 照家目录），要是一个在的目录，不落在数据根里工作区以外的地方
/// （数据根里只有工作区能当工作目录，`sessions::pick`）。
fn usable(core: &Core, cwd: &str) -> bool {
    let home = crate::workspace::home(core);
    let Ok(real) = miyu_fs::resolve(std::path::Path::new("/"), home.as_deref(), cwd) else {
        return false;
    };
    let (data_root, own) = crate::sessions::roots(core);
    real.is_dir() && (!miyu_fs::within(&real, &data_root) || miyu_fs::within(&real, &own))
}

/// 建、改出的错照协议说。
fn refusal(error: ScheduleError) -> Refusal {
    match error {
        ScheduleError::Cron(_) => Refusal::bad_schedule("cron"),
        ScheduleError::NoNext => Refusal::bad_schedule("past"),
        ScheduleError::TooShort => Refusal::bad_schedule("short"),
        ScheduleError::Unknown => Refusal::UNKNOWN_SCHEDULE,
        ScheduleError::Zone => Refusal::bad_schedule("zone"),
        ScheduleError::TooMany => Refusal::TOO_MANY_SCHEDULES,
        ScheduleError::Unusable | ScheduleError::Db(_) => Refusal::INTERNAL,
    }
}

/// 一项的写法（`docs/blueprint/schedules.md` 第五节第 4 条）。
pub(crate) fn entry(schedule: &Schedule) -> Value {
    let (when, recurring) = match &schedule.when {
        When::Cron {
            cron,
            recurring,
            zone,
        } => {
            let mut when = json!({"cron": cron, "recurring": recurring});
            if let Some(zone) = zone {
                when["zone"] = json!(zone);
            }
            (when, *recurring)
        }
        When::At { at } => (json!({"at": at}), false),
        When::Every { every } => (json!({"every": every}), true),
    };
    let target = match &schedule.target {
        Target::Session { session } => json!({"session": session}),
        Target::New {
            persona,
            preset,
            cwd,
        } => {
            json!({"new": {"persona": persona, "preset": preset, "cwd": cwd}})
        }
        Target::Package { .. } => json!({"package": {}}),
    };
    let by = match &schedule.by {
        Creator::Person => json!({"kind": "person"}),
        Creator::Model { session } => json!({"kind": "model", "session": session}),
        Creator::Package { package } => json!({"kind": "package", "package": package}),
    };
    let mut entry = json!({
        "id": schedule.id,
        "when": when,
        "prompt": schedule.prompt,
        "target": target,
        "by": by,
        "recurring": recurring,
        "paused": schedule.paused,
        "created": schedule.created,
    });
    if let Some(looping) = schedule.looping {
        entry["loop"] = json!(match looping {
            Loop::Fixed => "fixed",
            Loop::Dynamic => "dynamic",
        });
    }
    if let Some(next) = schedule.next {
        entry["next"] = json!(next);
    }
    if let Some(last) = &schedule.last {
        let mut written = json!({"at": last.at, "result": last.result});
        if let Some(session) = &last.session {
            written["session"] = json!(session);
        }
        entry["last"] = written;
    }
    entry
}
