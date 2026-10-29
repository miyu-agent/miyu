//! 交进会话（施工 J-1 上，设计 `45-定时和循环.md` 第三节第 2 条）：照「跑在哪」交给已有的会话，或者照建时定下的人格、预设、
//! 工作目录新开一个。交进去的那句记成 `by: schedule`（带任务编号），照「别处来的话」的规矩叫醒她：正在跑一轮的排在这一轮
//! 后面。
//!
//! - 她自己建的（`by` 是会话里的她）到点时看那个会话现在的预设：关了建它的那个包（定时任务看 `schedule`，循环看 `loop`）的
//!   这一次不跑，记 `skipped:preset_off`，下一次照算。人建的、包建的不看预设。
//! - 会话没了：记 `skipped:session_gone`，任务停掉，留给人看、给人删。

use std::sync::Arc;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::{self, By, ScheduleLoop};
use miyu_kernel::session::{Command, Outcome};
use miyu_kernel::time::Timestamp;
use miyu_store::schedules::{Creator, Loop, Schedule, Target};

use crate::Core;
use crate::sessions::Opening;

/// 定时任务的包：她用 `cron_create` 建的看它（施工 J-2 建清单）。
const SCHEDULE_PACKAGE: &str = "miyu-base-schedule";
/// 循环的包：`/loop`、`schedule_wakeup` 建的看它。
const LOOP_PACKAGE: &str = "miyu-base-loop";

/// 交的结果。
pub(super) struct Delivered {
    /// `ran`、`queued`、`skipped:<原因>`。
    pub(super) result: String,
    /// 交给了哪个会话。
    pub(super) session: Option<SessionId>,
    /// 任务要停掉（会话没了）。
    pub(super) stop: bool,
}

impl Delivered {
    fn skipped(why: &str, session: Option<SessionId>, stop: bool) -> Delivered {
        Delivered {
            result: format!("skipped:{why}"),
            session,
            stop,
        }
    }
}

/// 把 `schedule` 的这一次（到点在 `due`）交出去。
pub(super) async fn deliver(core: &Arc<Core>, schedule: &Schedule, due: Timestamp) -> Delivered {
    // 同一次的命令编号一样：核心重启后再交一遍，会话认得是重发。
    let base = format!("schedule/{}/{}", schedule.id, due.unix_millis());
    let by = By::Schedule(origin::Schedule {
        schedule: schedule.id.clone(),
        looping: schedule.looping.map(|looping| match looping {
            Loop::Fixed => ScheduleLoop::Fixed,
            Loop::Dynamic => ScheduleLoop::Dynamic,
        }),
    });
    let session = match &schedule.target {
        Target::Package { missed } => return pushed(core, schedule, due, *missed).await,
        Target::Session { session } => session.clone(),
        Target::New {
            persona,
            preset,
            cwd,
        } => match opened(core, &base, persona.as_deref(), preset, cwd).await {
            Some(session) => session,
            None => return Delivered::skipped("not_opened", None, false),
        },
    };
    let Ok(found) = core.sessions.get(core, &session).await else {
        return Delivered::skipped("session_gone", Some(session), true);
    };
    if let Creator::Model { .. } = schedule.by {
        let package = match schedule.looping {
            None => SCHEDULE_PACKAGE,
            Some(_) => LOOP_PACKAGE,
        };
        let preset = found
            .handle
            .current()
            .await
            .ok()
            .and_then(|current| current.preset);
        if !crate::presets::package_on(core, preset.as_deref(), package) {
            return Delivered::skipped("preset_off", Some(session), false);
        }
    }
    let Ok(id) = CommandId::parse(&format!("{base}/say")) else {
        return Delivered::skipped("bad_id", Some(session), false);
    };
    // 每次新开的跑完要告诉人（施工 J-3 下）：先订阅，交进去的那一轮的事件一定在订阅以后。
    let watching = match schedule.target {
        Target::New { .. } => found.handle.subscribe().await.ok(),
        Target::Session { .. } | Target::Package { .. } => None,
    };
    let busy = found.handle.busy();
    let send = Command::Send {
        blocks: vec![Block::Text(Text {
            text: schedule.prompt.clone(),
        })],
        urgent: false,
        venue: None,
    };
    match found.handle.command(id, by, send).await {
        Ok(Outcome::Accepted { events }) => {
            if let (Some(pushes), Some(said)) = (watching, events.first().copied()) {
                let which = (schedule.id.clone(), session.clone());
                core.schedules().watch_ran(which, said, pushes);
            }
            Delivered {
                result: if busy { "queued" } else { "ran" }.to_string(),
                session: Some(session),
                stop: false,
            }
        }
        Ok(_) => Delivered::skipped("refused", Some(session), false),
        Err(_) => {
            core.sessions.forget(&session).await;
            Delivered::skipped("session_gone", Some(session), true)
        }
    }
}

/// 推给建它的扩展（施工 J-4）：`schedule.fired` 交给那个包，它没连着的记 `skipped:package_unavailable`，一次性的照常删、周期的
/// 等下一次。
async fn pushed(core: &Arc<Core>, schedule: &Schedule, due: Timestamp, missed: u32) -> Delivered {
    let Creator::Package { package } = &schedule.by else {
        return Delivered::skipped("package_unavailable", None, false);
    };
    let Some(peer) = core.provided.linked(package) else {
        return Delivered::skipped("package_unavailable", None, false);
    };
    let mut fired = serde_json::json!({"id": schedule.id, "at": due, "prompt": schedule.prompt});
    if missed > 0 {
        fired["missed"] = serde_json::json!(missed);
    }
    if !peer.notify("schedule.fired", fired) {
        return Delivered::skipped("package_unavailable", None, false);
    }
    Delivered {
        result: "sent".to_string(),
        session: None,
        stop: false,
    }
}

/// 每次新开的：照建时定下的人格、预设、工作目录开一个，没人在场（要问人的照规矩拒）。开不成的记一行、没有。
async fn opened(
    core: &Arc<Core>,
    base: &str,
    persona: Option<&str>,
    preset: &str,
    cwd: &str,
) -> Option<SessionId> {
    let command = CommandId::parse(&format!("{base}/open")).ok()?;
    let who = Opening {
        attended: false,
        oneshot: false,
        model: None,
        venue: None,
        memory: None,
        preset: Some(preset.to_string()),
        group: None,
        profile: false,
        images: true,
        titles: true,
        origin: None,
        confined: false,
    };
    match core
        .sessions
        .create(
            core,
            command,
            Some(persona),
            cwd.to_string(),
            Vec::new(),
            who,
        )
        .await
    {
        Ok(created) => Some(created.id),
        Err(refusal) => {
            tracing::warn!(target: super::TARGET, reason = ?refusal, "schedule session not opened");
            None
        }
    }
}
