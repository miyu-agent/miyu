//! 交给会话的定时端口（施工 J-2，设计 `45-定时和循环.md` 第三节第 3、4 条）：`cron_create`、`cron_delete`、`cron_list`、
//! `schedule_next` 经它动唤醒表。造会话、载入时造一个，同软件包的端口。
//!
//! - 「这个会话上的」：她在这个会话里建的（`by` 是这个会话），和跑在这个会话上的（`target` 是这个会话，例如人敲的 `/loop`）。
//!   列、删照它；一个会话最多 [`PER_SESSION`] 个只数她建的。
//! - 动态循环是跑在这个会话上、`loop` 是 `dynamic` 的那一行（一个会话至多一行）：`schedule_next` 改它下一次的时刻和那句话，
//!   停的删掉它。
//! - 没开着动态循环的，`schedule_next` 挂一次「定时触发」（施工 FX-21，项目主人 2026-11-21 定，照 LiteMiyu LM-20）：她在这个
//!   会话里建的、某个时刻跑一次的那一行（`When::At`，没有 `loop`），到点照定时任务交进来、交过就删。一个会话至多一个：再定换掉，
//!   停的撤掉。要写到点交进来的话。

use std::sync::{Arc, Weak};

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;
use miyu_tool::{
    PER_SESSION, ScheduleAnswer, ScheduleAnswering, ScheduleAsk, SchedulePort, ScheduleRefused,
    Scheduled,
};

use super::{Creator, Loop, Schedule, ScheduleError, Target, Wanted, When};
use crate::Core;

/// 交给会话的那一份。
pub(crate) fn port(core: &Arc<Core>) -> Arc<dyn SchedulePort> {
    Arc::new(Port {
        core: Arc::downgrade(core),
    })
}

/// 核心那一头：她建的记在唤醒表里。
struct Port {
    core: Weak<Core>,
}

impl SchedulePort for Port {
    fn schedule(&self, session: &SessionId, ask: ScheduleAsk) -> ScheduleAnswering<'_> {
        let answered = self.answer(session, ask);
        Box::pin(async move { answered })
    }
}

impl Port {
    /// 办 `ask`：唤醒表读写都快，当场办完。
    fn answer(
        &self,
        session: &SessionId,
        ask: ScheduleAsk,
    ) -> Result<ScheduleAnswer, ScheduleRefused> {
        let core = self.core.upgrade().ok_or(ScheduleRefused::Unavailable)?;
        let schedules = core.schedules();
        let all = schedules.list().map_err(refused)?;
        let here: Vec<&Schedule> = all.iter().filter(|one| on(one, session)).collect();
        match ask {
            ScheduleAsk::Create {
                cron,
                prompt,
                recurring,
            } => {
                let made = here.iter().filter(|one| made_in(one, session)).count();
                if made >= PER_SESSION {
                    return Err(ScheduleRefused::Full);
                }
                let wanted = Wanted {
                    by: Creator::Model {
                        session: session.clone(),
                    },
                    when: When::Cron {
                        cron,
                        recurring,
                        zone: None,
                    },
                    prompt,
                    target: Target::Session {
                        session: session.clone(),
                    },
                    looping: None,
                    first: None,
                };
                let added = schedules.add(wanted).map_err(refused)?;
                Ok(ScheduleAnswer::Created(shown(&added)))
            }
            ScheduleAsk::Delete { id } => {
                let one = here
                    .iter()
                    .find(|one| one.id == id)
                    .ok_or(ScheduleRefused::NotFound)?;
                let gone = shown(one);
                schedules.cancel(&id).map_err(refused)?;
                Ok(ScheduleAnswer::Deleted(gone))
            }
            ScheduleAsk::List => Ok(ScheduleAnswer::Listed(
                here.iter().map(|one| shown(one)).collect(),
            )),
            ScheduleAsk::Wakeup { delay, prompt } => {
                let millis = i64::try_from(delay.saturating_mul(1000)).unwrap_or(i64::MAX);
                // 没有上限（施工 WK-1）：写不下的照最晚的时刻。
                let next = Timestamp::from_unix_millis(
                    schedules.now().unix_millis().saturating_add(millis),
                )
                .unwrap_or(Timestamp::LATEST);
                if let Some(looping) = dynamic(&here, session) {
                    let woken = schedules
                        .set_next(&looping.id, Some(next), prompt)
                        .map_err(refused)?;
                    return Ok(ScheduleAnswer::Woken(shown(&woken)));
                }
                let prompt = prompt.ok_or(ScheduleRefused::NoPrompt)?;
                let pending = triggered(&here, session);
                let made = here.iter().filter(|one| made_in(one, session)).count();
                if pending.is_none() && made >= PER_SESSION {
                    return Err(ScheduleRefused::Full);
                }
                let wanted = Wanted {
                    by: Creator::Model {
                        session: session.clone(),
                    },
                    when: When::At { at: next },
                    prompt,
                    target: Target::Session {
                        session: session.clone(),
                    },
                    looping: None,
                    first: None,
                };
                let added = schedules.add(wanted).map_err(refused)?;
                // 先加新的再撤旧的：加不成时原来挂着的还在。
                if let Some(old) = pending {
                    schedules.cancel(&old.id).map_err(refused)?;
                }
                Ok(ScheduleAnswer::Woken(shown(&added)))
            }
            ScheduleAsk::StopLoop => {
                let stopping = dynamic(&here, session)
                    .or_else(|| triggered(&here, session))
                    .ok_or(ScheduleRefused::NoLoop)?;
                schedules.cancel(&stopping.id).map_err(refused)?;
                Ok(ScheduleAnswer::LoopStopped)
            }
        }
    }
}

/// 在这个会话上：她在这里建的，或者跑在这里的。
fn on(one: &Schedule, session: &SessionId) -> bool {
    made_in(one, session)
        || matches!(&one.target, Target::Session { session: target } if target == session)
}

/// 她在这个会话里建的。
fn made_in(one: &Schedule, session: &SessionId) -> bool {
    matches!(&one.by, Creator::Model { session: by } if by == session)
}

/// 跑在这个会话上的动态循环那一行。
fn dynamic<'a>(here: &[&'a Schedule], session: &SessionId) -> Option<&'a Schedule> {
    here.iter().copied().find(|one| {
        one.looping == Some(Loop::Dynamic)
            && matches!(&one.target, Target::Session { session: target } if target == session)
    })
}

/// 她在这个会话里挂的定时触发（施工 FX-21）：她建的、某个时刻跑一次、不是循环的那一行。`cron_create` 建的写的是 cron，
/// 不会是这种。
fn triggered<'a>(here: &[&'a Schedule], session: &SessionId) -> Option<&'a Schedule> {
    here.iter().copied().find(|one| {
        made_in(one, session) && one.looping.is_none() && matches!(one.when, When::At { .. })
    })
}

/// 给她看的那几样。
fn shown(one: &Schedule) -> Scheduled {
    let (when, recurring) = match &one.when {
        When::Cron {
            cron, recurring, ..
        } => (cron.clone(), *recurring),
        When::Every { every } => (format!("every {every}s"), true),
        When::At { at } => (at.to_string(), false),
    };
    Scheduled {
        id: one.id.clone(),
        when,
        prompt: one.prompt.clone(),
        recurring,
        kind: one.looping.map(|looping| match looping {
            Loop::Fixed => "fixed",
            Loop::Dynamic => "dynamic",
        }),
        next: one.next,
        paused: one.paused,
    }
}

/// 唤醒表的错照她看得懂的说：cron 写错的带原话，没有的是没找到，别的是这一回用不了。
fn refused(error: ScheduleError) -> ScheduleRefused {
    match error {
        ScheduleError::Cron(why) => ScheduleRefused::BadCron(why),
        ScheduleError::Unknown => ScheduleRefused::NotFound,
        ScheduleError::NoNext => ScheduleRefused::BadCron("no future time matches".to_string()),
        ScheduleError::TooMany => ScheduleRefused::AllFull,
        ScheduleError::TooShort
        | ScheduleError::Zone
        | ScheduleError::LoopKind
        | ScheduleError::Unusable
        | ScheduleError::Db(_) => ScheduleRefused::Unavailable,
    }
}
