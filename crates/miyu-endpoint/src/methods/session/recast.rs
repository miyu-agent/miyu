//! `session.configure` 换人格、预设（施工 P-13，`docs/designs/16-人格与预设.md` Y5）：人格、预设要在，子代理不能换（`data.field`
//! 是写了的那一格）。内核记下要换成的、马上压缩一轮，下一个回合开始真换；有回合在跑的拒 `turn_running`。命令编号照这条请求
//! 的接 `/recast`：模型那一条已经用了原来的。

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::session::{Command, Outcome};
use miyu_session::Handle;

use crate::Core;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 查人格、预设在不在，会话能不能换，交回要发的命令；两样都没写的没有。
pub(super) async fn checked(
    core: &Arc<Core>,
    session: &SessionId,
    (persona, preset): (Option<Option<&str>>, Option<&str>),
) -> Result<Option<Command>, Refusal> {
    if persona.is_none() && preset.is_none() {
        return Ok(None);
    }
    if let Some(Some(id)) = persona {
        crate::personas::resolve(core, Some(Some(id))).await?;
    }
    // 空的是换成无工具（施工 P-16 下）：不用找。
    if let Some(id) = preset.filter(|id| !id.is_empty()) {
        crate::presets::resolve(core, Some(id)).await?;
    }
    if subagent(core, session).await? {
        let field = if persona.is_some() {
            "persona"
        } else {
            "preset"
        };
        return Err(Refusal::bad_field(field));
    }
    Ok(Some(Command::Recast {
        persona: persona.map(|persona| persona.map(str::to_string)),
        preset: preset.map(str::to_string),
    }))
}

/// 会话 `session` 是不是派出去的子代理：有父会话、不是分叉出来的。会话没有、读不了第一条的照会话没有。
async fn subagent(core: &Core, session: &SessionId) -> Result<bool, Refusal> {
    let owner = core
        .sessions
        .owner(core, session)
        .await
        .ok_or(Refusal::NOT_FOUND)?;
    let dir = core.root.session_dir(&owner, session);
    let first = tokio::task::spawn_blocking(move || miyu_store::log::first_event(&dir))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|_| Refusal::NOT_FOUND)?;
    Ok(match first.body {
        Body::SessionCreated(created) => created.parent.is_some() && created.forked.is_none(),
        _ => false,
    })
}

/// 发换人格、预设的命令：拒绝的是内核的原因码；会话停了的从表里拿掉。
pub(super) async fn send(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    command: Command,
) -> Result<(), Refusal> {
    let id = CommandId::parse(&format!("{}/recast", request.id.as_str()))
        .map_err(|_| Refusal::BAD_PARAMS)?;
    match handle
        .command(id, crate::sessions::admin(core), command)
        .await
    {
        Ok(Outcome::Rejected { reason, about }) => Err(Refusal::kernel_about(reason, &about)),
        Ok(_) => Ok(()),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}
