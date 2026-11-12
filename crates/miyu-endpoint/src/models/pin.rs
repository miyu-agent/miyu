//! `session.create` 带的钉在会话上的几样（施工 VM-1 的看图模型，施工 CT-1 的压缩触发线）：造好以后照先后发给会话，各记一条。
//! 交给内核的命令编号照这条请求的接 `/<名字>`（`/vision`、`/compaction_line`）：造会话那一条已经用了原来的。

use std::sync::Arc;

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::session::{Command, Outcome};

use crate::Core;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 造好以后给会话 `session` 照先后发 `commands`（名字和查好的命令，没有命令的跳过），交回记下的那几条的序号。
///
/// # Errors
///
/// 命令编号接不成：`bad_params`。会话停了：`stopped`。内核拒了的照它的原因码（照理不会：刚造的会话）。
pub(crate) async fn pin(
    core: &Arc<Core>,
    request: &Request,
    session: &SessionId,
    commands: impl IntoIterator<Item = (&'static str, Option<Command>)>,
) -> Result<Vec<u64>, Refusal> {
    let mut events = Vec::new();
    for (suffix, command) in commands {
        let Some(command) = command else {
            continue;
        };
        let id = CommandId::parse(&format!("{}/{suffix}", request.id.as_str()))
            .map_err(|_| Refusal::BAD_PARAMS)?;
        let found = core.sessions.get(core, session).await?;
        match found
            .handle
            .command(id, crate::sessions::person(), command)
            .await
        {
            Ok(Outcome::Accepted { events: seqs }) => {
                events.extend(seqs.iter().map(|seq| seq.get()));
            }
            Ok(Outcome::Rejected { reason, about }) => {
                return Err(Refusal::kernel_about(reason, &about));
            }
            Ok(_) => return Err(Refusal::INTERNAL),
            Err(_) => {
                core.sessions.forget(session).await;
                return Err(Refusal::STOPPED);
            }
        }
    }
    Ok(events)
}
