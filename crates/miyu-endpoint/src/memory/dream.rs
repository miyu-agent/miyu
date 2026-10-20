//! 现在就整理记忆（施工 R-7 补，`docs/blueprint/memory.md` 第七条第 9 款，`protocol.md` 的 `memory.dream`、`/dream`）：带
//! `session` 的叫那个会话先把还没抽的抽了再合（[`Handle::dream`]），带 `persona`、都不写的照那一间直接合
//! （[`miyu_session::Keeper::dream`]）。交回几样数；这一间正在合的 `memory_busy`，没合成的 `dream_failed`。几秒到十几秒，
//! 在后台答（`methods::answered_later`）。`memory.dream_stop` 叫停这一间人叫的那一趟（施工 R-23），照顺序答。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_session::{Dreamed, Handle, NotDreamed};

use super::params::Where;
use super::{find, installed, read, remembers, session_id, using};
use crate::Core;
use crate::refusal::Refusal;
use crate::sessions::offset;

/// `memory.dream`：照参数找哪一间（同 `memory.*`），现在就整理，交回 `{"given", "revised", "retired", "summary"}`；会话的
/// `/stop` 叫停了的多一个 `"stopped": true`（施工 R-22），前面几样是叫停以前合完了的；跑在整理会话里的多 `"session"`（施工 D-2）。
pub(crate) async fn call(core: &Arc<Core>, params: &Value) -> Result<Value, Refusal> {
    if !installed(core) {
        return Err(Refusal::MEMORY_NOT_INSTALLED);
    }
    let at: Where = read(params)?;
    let dreamed = match (&at.session, &at.persona, &at.as_external) {
        (Some(session), None, None) => {
            let session = session_id(session)?;
            let found = core.sessions.get(core, &session).await?;
            in_session(core, &session, &found.handle).await?
        }
        // 别的组合（写错的也在里面）照 `find` 判。
        _ => {
            let keeper = find(core, at).await?;
            // 合并的请求照这时的配置发，记在管理员名下（M8 只有他，和 `model.call` 一样）。
            let using = using(core);
            answered(keeper.dream(using.config, using.owner, offset()).await)?
        }
    };
    let mut answer = json!({
        "given": dreamed.given,
        "revised": dreamed.revised,
        "retired": dreamed.retired,
        "summary": dreamed.summary,
    });
    if dreamed.stopped {
        answer["stopped"] = Value::Bool(true);
    }
    // 这一趟跑在哪个整理会话里（施工 D-2，设计 41）：头照它跳到那个会话；没有要整理的、没接会话表的不写。
    if let Some(session) = &dreamed.session {
        answer["session"] = Value::String(session.to_string());
    }
    Ok(answer)
}

/// 会话 `session`（句柄 `handle`）现在就整理（带 `session` 的 `memory.dream`、`/dream`）：那个会话先抽、再合。
///
/// # Errors
///
/// 这个会话没有记忆：`memory_unavailable`；会话停了：`stopped`（从会话表里拿掉）；别的照 [`answered`]。
pub(crate) async fn in_session(
    core: &Core,
    session: &SessionId,
    handle: &Handle,
) -> Result<Dreamed, Refusal> {
    if !remembers(core, handle) {
        return Err(Refusal::MEMORY_UNAVAILABLE);
    }
    match handle.dream().await {
        Ok(dreamed) => answered(dreamed),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 会话那一层交回的写成拒绝：正在合的 `memory_busy`，没有记忆可整理的 `memory_unavailable`，没合成的 `dream_failed`。
fn answered(dreamed: Result<Dreamed, NotDreamed>) -> Result<Dreamed, Refusal> {
    dreamed.map_err(|not| match not {
        NotDreamed::Busy => Refusal::MEMORY_BUSY,
        NotDreamed::Off => Refusal::MEMORY_UNAVAILABLE,
        NotDreamed::Failed(why) => Refusal::dream_failed(why),
    })
}

/// `memory.dream_stop`（施工 R-23，`memory.md` 第七条第 13 款）：照参数找哪一间（同 `memory.*`），叫停这时人叫的整理，不管是
/// 哪一路叫的（会话里的 `/dream`、`memory.dream`），定时的不停。交回 `{"stopped": true}`；被叫停的那条照样回应，带
/// `"stopped": true`。
///
/// # Errors
///
/// 这一间这时没有人叫的整理：`not_dreaming`；找哪一间的拒绝同 `memory.*`。
pub(crate) async fn stop(core: &Arc<Core>, params: &Value) -> Result<Value, Refusal> {
    let keeper = find(core, read(params)?).await?;
    match keeper.stop_dream() {
        true => Ok(json!({"stopped": true})),
        false => Err(Refusal::NOT_DREAMING),
    }
}

#[cfg(test)]
mod tests;
