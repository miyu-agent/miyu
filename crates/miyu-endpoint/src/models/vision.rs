//! 会话钉的看图模型（施工 VM-1，`docs/blueprint/models.md`「协议」、「怎么走」第十三条第 4 条）：`session.configure`、
//! `session.create` 的 `vision` 共用的查法和发法。
//!
//! 写法、解析同 `model`（[`super::record`]：照这时不算项目配置的最终值，解析不出的 `unknown_model`），`null` 是去掉、回到
//! 跟着 `models.vision`。交给内核的命令编号照这条请求的接 `/vision`：模型那一条已经用了原来的。

use std::sync::Arc;

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::session::{Command, Outcome};

use crate::Core;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 要交给内核的命令：`text` 外层没有的是不换（没有命令），里面没有的是去掉；钉成的引用先照这时的配置解析好。
///
/// # Errors
///
/// 解析不出：`unknown_model`，原话记一行 `DEBUG unknown model`。
pub(crate) fn command(core: &Core, text: Option<Option<&str>>) -> Result<Option<Command>, Refusal> {
    let Some(text) = text else {
        return Ok(None);
    };
    let vision = text.map(|text| super::record(core, text)).transpose()?;
    Ok(Some(Command::SetVision { vision }))
}

/// `session.create` 带的：造好以后给会话 `session` 发 `command`（[`command`] 交回的），交回记下的那几条的序号；没有命令的
/// 什么都不发，交回空的。
///
/// # Errors
///
/// 命令编号接不成：`bad_params`。会话停了：`stopped`。内核拒了的照它的原因码（照理不会：刚造的会话）。
pub(crate) async fn pin(
    core: &Arc<Core>,
    request: &Request,
    session: &SessionId,
    command: Option<Command>,
) -> Result<Vec<u64>, Refusal> {
    let Some(command) = command else {
        return Ok(Vec::new());
    };
    let id = CommandId::parse(&format!("{}/vision", request.id.as_str()))
        .map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, session).await?;
    match found
        .handle
        .command(id, crate::sessions::person(), command)
        .await
    {
        Ok(Outcome::Accepted { events }) => Ok(events.iter().map(|seq| seq.get()).collect()),
        Ok(Outcome::Rejected { reason, about }) => Err(Refusal::kernel_about(reason, &about)),
        Ok(_) => Err(Refusal::INTERNAL),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}
