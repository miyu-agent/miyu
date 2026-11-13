//! 结束一段人生（施工 LF-4 上，设计 `70-推演人生.md` 第三节第 5 条，`docs/blueprint/life.md`「结束」）：主人的 `life.kill`、
//! `life.new` 和她自己的 `end_life`（LF-4 下）做同样的几件事，照这个先后，每一步先查后做、能重做：
//!
//! 1. 记 `life.ended`（命令编号照会话固定：`life/<会话>/end`，重做时内核照编号认出是同一个，不记第二条）；主人结束、这时正跑
//!    一轮的，打断它。闲着的内核当场封存，在跑的这一轮结束时封存（`session/fork.md`「封存」）。
//! 2. 等封存了再清：那一轮里记下的、进回合索引的都不漏。
//! 3. 删掉定时表里人生那一行。
//! 4. 清记忆：出处全在这个会话里的（现成的按会话清，命令编号照会话固定：重做照编号只算一次）。
//! 5. 回合索引里拿掉这个会话的全部，埋整个会话的墓碑（载入、补齐照日志判，`memory.md` 第一条第 7 款）。
//! 6. 给主人推一条通知 `life.ended`：只在第 1 步是这一次新记上的时候推。
//!
//! 第 1 步当场做完交回；第 2 到 6 步在自己的任务里做（她调工具结束的，这一轮要等工具答完才结束、才封存），调的一方要等的
//! 等 [`Finishing::done`]。

use std::sync::Arc;

use serde_json::json;
use tokio::task::JoinHandle;

use miyu_kernel::event::{Body, LifeEnded};
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Appended, Command, Outcome, Queued};
use miyu_session::{Handle, Pushed};

use crate::Core;
use crate::refusal::Refusal;
use crate::schedules::methods::refusal as schedule_refusal;

/// 结束的通知（施工 LF-4 上，`schedules.md` 第七节）。
const ENDED: &str = "life.ended";

/// 在路上的结束：第 1 步记下了，后面几步在自己的任务里做。
pub(crate) struct Finishing {
    /// 第 2 到 6 步。
    cleanup: JoinHandle<Result<(), Refusal>>,
}

impl Finishing {
    /// 等后面几步做完。
    ///
    /// # Errors
    ///
    /// 哪一步没成：照那一步的原因，重做（再结束一次）接着清完。
    pub(crate) async fn done(self) -> Result<(), Refusal> {
        self.cleanup.await.unwrap_or(Err(Refusal::INTERNAL))
    }
}

/// 结束会话 `session` 上的人生：`by` 是人，或者她自己的那次工具调用（`By::Tool`）；遗言只有她自己结束的才有。已经结束了的
/// 照重做：跳过记的那一步，后面几步再走一遍。见模块的说明。
///
/// # Errors
///
/// 会话找不到、停了；内核拒了 `life.ended`。
pub(crate) async fn finish(
    core: &Arc<Core>,
    session: &SessionId,
    by: By,
    last_words: Option<String>,
) -> Result<Finishing, Refusal> {
    let handle = core.sessions.get(core, session).await?.handle;
    let now = handle.current().await.map_err(|_| Refusal::STOPPED)?;
    let recorded = !now.life_ended;
    if recorded {
        let ended = Command::Append {
            event: Appended::LifeEnded(LifeEnded {
                last_words: last_words.clone(),
            }),
        };
        send(core, &handle, step(session, "end")?, by.clone(), ended).await?;
        // 主人结束的，这时正跑的那一轮打断（她自己结束的，那一轮照常说完）。排着的留着，不再开轮。
        if !matches!(by, By::Tool(_)) && handle.busy() {
            let stop = Command::Interrupt {
                queued: Queued::Keep,
            };
            send(core, &handle, step(session, "stop")?, by.clone(), stop).await?;
        }
    }
    let notice = recorded.then(|| {
        json!({
            "persona": now.persona,
            "session": session.as_str(),
            "by": match by { By::Tool(_) => "self", _ => "owner" },
            "last_words": last_words,
        })
    });
    let (core, session) = (Arc::clone(core), session.clone());
    let cleanup = tokio::spawn(async move { clean(&core, &session, by, notice).await });
    Ok(Finishing { cleanup })
}

/// 第 2 到 6 步。
async fn clean(
    core: &Arc<Core>,
    session: &SessionId,
    by: By,
    notice: Option<serde_json::Value>,
) -> Result<(), Refusal> {
    let handle = sealed(core, session).await?;
    if let Some(row) = super::row_of(core, session)? {
        match core.schedules().cancel(&row.id) {
            Ok(()) | Err(crate::schedules::ScheduleError::Unknown) => {}
            Err(error) => return Err(schedule_refusal(error)),
        }
    }
    match crate::memory::clear_in(core, &handle, &step(session, "memory")?, by).await {
        Ok(cleared) => {
            tracing::info!(target: "miyu::endpoint", session = session.as_str(), cleared, "life memory cleared");
        }
        // 这个会话没有记忆（预设关了、范围 `off`）：没有要清的。
        Err(refusal) if refusal.reason == "memory_unavailable" => {}
        Err(refusal) => return Err(refusal),
    }
    let recall = Arc::clone(&core.memory.turns);
    let forgot = session.clone();
    let forgotten = tokio::task::spawn_blocking(move || recall.forget_session(&forgot))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    if let Err(error) = forgotten {
        tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "memory index not updated");
        return Err(Refusal::INTERNAL);
    }
    if let Some(more) = notice {
        let more = without_nulls(more);
        core.schedules()
            .notices()
            .send(ENDED, core.schedules().now(), more);
    }
    tracing::info!(target: "miyu::endpoint", session = session.as_str(), "life ended");
    Ok(())
}

/// 等会话 `session` 封存了：先订阅再问，问的那一刻已经封存的不等；推送跟不上、会话停了的重新载入再问。
async fn sealed(core: &Arc<Core>, session: &SessionId) -> Result<Handle, Refusal> {
    loop {
        let handle = core.sessions.get(core, session).await?.handle;
        let Ok(mut pushes) = handle.subscribe().await else {
            continue;
        };
        if handle.current().await.is_ok_and(|now| now.sealed) {
            return Ok(handle);
        }
        while let Ok(pushed) = pushes.next().await {
            let Pushed::Events(events) = &*pushed else {
                continue;
            };
            if events
                .iter()
                .any(|event| matches!(event.body, Body::SessionSealed(_)))
            {
                return Ok(handle);
            }
        }
    }
}

/// 结束这件事的第几步的命令编号：照会话固定（`life/<会话>/<步>`），重做时内核、记忆照编号认出是同一个，只算一次。
fn step(session: &SessionId, name: &str) -> Result<CommandId, Refusal> {
    CommandId::parse(&format!("life/{}/{name}", session.as_str())).map_err(|_| Refusal::INTERNAL)
}

/// 照 `id` 给会话发一个命令。拒绝的照内核的原因码；会话停了的回 `stopped`。
async fn send(
    core: &Core,
    handle: &Handle,
    id: CommandId,
    by: By,
    command: Command,
) -> Result<(), Refusal> {
    match handle.command(id, by, command).await {
        Ok(Outcome::Rejected { reason, about }) => Err(Refusal::kernel_about(reason, &about)),
        Ok(_) => Ok(()),
        Err(_) => {
            core.sessions.forget(handle.id()).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 去掉值是 `null` 的几格：没有人格、没有遗言的不写。
fn without_nulls(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
    value
}
