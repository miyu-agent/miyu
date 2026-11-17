//! 人生会话自己里面的 `/loop`、`/stop`（施工 LF-13，`docs/blueprint/life.md`「人生会话里的 `/loop`、`/stop`」；项目主人
//! 2026-11-17 本机验收第 23 条，定的是人生就是这个会话的循环）：
//!
//! - 光打 `/loop`：回人生的状态（同 `/life`）。
//! - `/loop off`（`clear`、`stop`）：暂停人生（同 `/life pause`）。暂停不是结束：那一行还在、记忆不清，`/life resume` 接着过。
//! - `/loop <话>`、带间隔的：不收，回一句。以前会在人生那一行旁边另建一行循环，她定下一次时定到了人生那一行上，新的那一行
//!   这一轮结束就被删掉；人想说什么直接发消息，她照样看得到。
//! - `/stop`：是全停，除了打断这一轮、停掉派出去的，还暂停人生；以前只打断，一小时里她又自己醒。
//!
//! 照会话认（它是没结束的人生会话），不经人格找：会话这时带不带人格都一样。结束只有 `/life kill`、`/life new` 和她自己的
//! `end_life`。

use std::sync::Arc;

use miyu_kernel::id::SessionId;
use miyu_session::Handle;

use super::{PAUSED, ran, say, told};
use crate::Core;
use crate::commands::Said;
use crate::commands::looping::Asked;
use crate::commands::registry::{Call, Ran};
use crate::refusal::Refusal;

/// 这个会话是没结束的人生会话。
///
/// # Errors
///
/// 会话停了：`stopped`。
pub(in crate::commands) async fn living(handle: &Handle) -> Result<bool, Refusal> {
    let current = handle.current().await.map_err(|_| Refusal::STOPPED)?;
    Ok(current.life && !current.life_ended)
}

/// 人生会话里的 `/loop`：见模块的说明。
pub(in crate::commands) async fn looped(
    call: &Call<'_>,
    asked: &Asked<'_>,
) -> Result<Ran, Refusal> {
    let now = match asked {
        Asked::Autonomous => crate::life::here(call.core, call.session)?,
        Asked::Off => crate::life::pause_here(call.core, call.session).await?,
        Asked::Fixed { .. } | Asked::Dynamic { .. } => {
            return Ok(ran(Said::plain("commands/loop-life"), None));
        }
    };
    // 那一行不在的也算停着（同 `life.status`）：没有说不出状态的时候。
    let said = told(call.core, &now, None)
        .await
        .unwrap_or_else(|| say(PAUSED, None, Vec::new()));
    Ok(ran(said, None))
}

/// `/stop` 打在会话 `session` 上：它是没结束的人生会话的，暂停人生，交回真。不是的交回假。暂停不成的记一行、照没停回：
/// `/stop` 别的几样（打断、停后台的）照样做。
pub(in crate::commands) async fn stopped(
    core: &Arc<Core>,
    session: &SessionId,
    handle: &Handle,
) -> bool {
    if !matches!(living(handle).await, Ok(true)) {
        return false;
    }
    match crate::life::pause_here(core, session).await {
        Ok(_) => true,
        Err(refusal) => {
            tracing::warn!(target: "miyu::endpoint", reason = refusal.reason, "life not paused");
            false
        }
    }
}
