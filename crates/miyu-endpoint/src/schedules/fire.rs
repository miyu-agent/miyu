//! 交进会话（施工 J-1 上，设计 `45-定时和循环.md` 第三节第 2 条）：照「跑在哪」交给已有的会话，或者照建时定下的人格、预设、
//! 工作目录新开一个。交进去的那句记成 `by: schedule`（带任务编号），照「别处来的话」的规矩叫醒她：正在跑一轮的排在这一轮
//! 后面。跑在已有会话上的循环不交进正在跑的这一轮，等它结束再交（施工 LO-2，[`waits`]、`busy.rs`）；撤回没处理时也这样等
//! （施工 FX-13），人刚敲的 `/loop` 的第一次除外：交 `Kickoff`，同人发一句话，由它开一轮、清掉撤销（施工 FX-14，[`kickoff`]）。
//! 她自己定节奏的循环交进去开的那一轮结束时没定下一次的，删掉这一行（施工 LO-3，`ended.rs`）。
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
use miyu_session::new_id;
use miyu_store::schedules::{Creator, Loop, Schedule, Target};

use crate::Core;
use crate::sessions::Opening;

/// 定时任务的包：她用 `cron_create` 建的看它（施工 J-2 建清单）。
const SCHEDULE_PACKAGE: &str = "base-schedule";
/// 循环的包：`/loop`、`schedule_next` 建的看它。
const LOOP_PACKAGE: &str = "base-loop";

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

/// 跑在已有会话上的循环（固定的、她自己定节奏的）到点时那个会话正在跑一轮（施工 LO-2），或者还有能恢复的撤销（施工 FX-13）：
/// 记下等它（`busy.rs`），交回真，这一次先不交、表不动。先记再看：看完到记下之间变了的也叫得醒。会话找不到的交回假，照常交
/// （交的时候记没了）。撤销没处理时以前照常交，内核只记下（`can_wake`），日志里多一条没人回答的话，`/loop off` 以后恢复撤销
/// 还由它开一轮。人刚敲的 `/loop` 的第一次（[`kickoff`]）不因撤销等（施工 FX-14）。
pub(super) async fn waits(core: &Arc<Core>, schedule: &Schedule) -> bool {
    let (Some(Loop::Fixed | Loop::Dynamic), Target::Session { session }) =
        (schedule.looping, &schedule.target)
    else {
        return false;
    };
    let Ok(found) = core.sessions.get(core, session).await else {
        return false;
    };
    let waiting = &core.schedules.waiting;
    waiting.hold(&schedule.id, session);
    if found.handle.busy() || (found.handle.restorable() && !kickoff(schedule)) {
        return true;
    }
    waiting.drop_one(&schedule.id);
    false
}

/// 人刚敲的 `/loop` 的第一次（施工 FX-14，2026-11-22 项目主人定）：人建的、跑在已有会话上、还没跑过的循环。只有 `/loop`
/// 建人建的循环，第一次就是敲的那一刻（正忙的等这一轮结束，表不动、`last` 还是空的）。人亲手敲命令同人发一句话，接受了
/// 撤回：交 `Kickoff`，还能恢复撤销也由它开一轮。之后每一次照旧，撤回没处理时搁着（施工 FX-13）。
fn kickoff(schedule: &Schedule) -> bool {
    matches!(schedule.by, Creator::Person)
        && matches!(schedule.looping, Some(Loop::Fixed | Loop::Dynamic))
        && matches!(schedule.target, Target::Session { .. })
        && schedule.last.is_none()
}

/// 循环交进去的正文就是出厂的那一句（施工 FX-15，光打 `/loop` 的自主循环）：`by` 写 `builtin`，头照它画「由 AI 自己决定」。
/// 照这一行现在的 `prompt` 和出厂的原文（去掉行尾空白，同 `commands/looping.rs` 存的）比；读不出的、定时任务不写。
async fn builtin(core: &Core, schedule: &Schedule) -> bool {
    if schedule.looping.is_none() {
        return false;
    }
    let resources = core.resources.clone();
    let read = tokio::task::spawn_blocking(move || resources.loop_autonomous()).await;
    matches!(read, Ok(Ok(text)) if text.trim_end() == schedule.prompt)
}

/// 把 `schedule` 的这一次（到点在 `due`）交出去。
pub(super) async fn deliver(core: &Arc<Core>, schedule: &Schedule, due: Timestamp) -> Delivered {
    // 每一次到点新造一个编号（施工 FX-2，`schedules.md` 第三节第 4 条）：以前用到点的毫秒，系统时间往回拨以后同一时刻又到点，
    // 编号和上一次一样，内核当成重发、不开轮。开会话的 `…/open` 和交话的 `…/say` 共用这一个。
    let base = format!("schedule/{}/{}", schedule.id, new_id(due).as_str());
    let by = By::Schedule(origin::Schedule {
        schedule: schedule.id.clone(),
        looping: schedule.looping.map(|looping| match looping {
            Loop::Fixed => ScheduleLoop::Fixed,
            Loop::Dynamic => ScheduleLoop::Dynamic,
        }),
        person: matches!(schedule.by, Creator::Person),
        builtin: builtin(core, schedule).await,
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
    // 每次新开的跑完要告诉人（施工 J-3 下），她自己定节奏的那一轮结束时看她定没定下一次（施工 LO-3，`ended.rs`）：先订阅，
    // 交进去的那一轮的事件一定在订阅以后。
    let watching = match (&schedule.target, schedule.looping) {
        (Target::New { .. }, _) | (Target::Session { .. }, Some(Loop::Dynamic)) => {
            found.handle.subscribe().await.ok()
        }
        (Target::Session { .. } | Target::Package { .. }, _) => None,
    };
    let busy = found.handle.busy();
    let blocks = vec![Block::Text(Text {
        text: schedule.prompt.clone(),
    })];
    let send = match kickoff(schedule) {
        // 人敲的那一行一起交（施工 FX-15）：头照它显示「/loop …」。
        true => Command::Kickoff {
            blocks,
            command: core.schedules.take_typed(&session),
        },
        false => Command::Send {
            blocks,
            urgent: false,
            venue: None,
        },
    };
    match found.handle.command(id, by, send).await {
        Ok(Outcome::Accepted { events }) => {
            if let (Some(pushes), Some(said)) = (watching, events.first().copied()) {
                match schedule.target {
                    Target::New { .. } => {
                        let which = (schedule.id.clone(), session.clone());
                        core.schedules().watch_ran(which, said, pushes);
                    }
                    // 正忙的排进了这一轮，不是它自己开的一轮：不盯。
                    _ if busy => {}
                    _ => {
                        let mine = super::ended::Mine::Said(said);
                        super::ended::watch(core, schedule.id.clone(), mine, pushes);
                    }
                }
            }
            // 内核回的是那一句，由它开了一轮的连同那条 `turn.started`（施工 SC-1）：开了的记 `ran`；没开、正在跑一轮的是排
            // 进了这一轮，`queued`；没开也没在跑的是只记下了（没人看着的一次性会话里她自己建的、还能恢复撤销的……），`held`。
            let result = match (events.len() > 1, busy) {
                (true, _) => "ran",
                (false, true) => "queued",
                (false, false) => "held",
            };
            Delivered {
                result: result.to_string(),
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
        voice_max_ms: None,
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
