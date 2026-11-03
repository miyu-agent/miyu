//! `session.fork {session}`（施工 C-8 上，`docs/blueprint/session/fork.md`）：从会话 `session` 分叉出一个子会话，历史引用它到
//! 分叉点为止的有效历史。分叉点由核心定：父会话在跑一轮的，取这一轮 `turn.started` 之前那一条；闲着的取最后一条。子会话的
//! 场所、关不关起来、工作目录、权限、策略快照、钉住的模型照父会话那一刻的。回应 `{session, upto}`。记成谁同 `events.append`。
//! 父会话不是主会话的（子代理、分叉出来的）`bad_params`；没有的 `not_found`。`seal_after`（施工 C-8 下）写进子会话的 `forked`：
//! 跑满这几轮封存，内核数、内核封。
//!
//! 写完第一条马上载入支线：分叉那一遍读好的底子（`miyu_session::ForkedBase`，施工 V-39 下）直接交给这一次载入，载入、
//! 支线的回合索引都不再读父会话。只交这一次：支线已经在会话表里的、载入不成的，它跟着放掉，以后再载入照常自己读。

use std::num::NonZeroU32;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_session::{Fork, ForkError, fork_handing, new_id};
use miyu_store::log::OpenError;

use crate::Core;
use crate::appending::recorder;
use crate::hello::Caller;
use crate::refusal::Refusal;
use crate::sessions::now;
use crate::wire::Request;

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// `session.fork` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ForkParams {
    session: String,
    /// 跑完几轮封存（施工 C-8 下）：1 起，`0` 读不进来就是 `bad_params`。
    seal_after: Option<NonZeroU32>,
}

/// `session.fork`：见模块的说明。
pub(crate) async fn fork(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
    params: ForkParams,
) -> Result<Value, Refusal> {
    let parent = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    core.sessions.get(core, &parent).await?;
    let by = recorder(caller)?;
    let cause = request.id.clone();
    let at = now();
    let child = new_id(at);
    let (upto, base) = fork_handing(Fork {
        root: &core.root,
        parent: &parent,
        id: child.clone(),
        cause,
        by,
        at,
        index: Some(core.index()),
        seal_after: params.seal_after,
        within: None,
    })
    .await
    .map_err(|error| refused(&parent, &error))?;
    core.sessions.get_forked(core, &child, base).await?;
    core.listing.changed(core, child.clone());
    tracing::info!(target: TARGET, parent = parent.as_str(), session = child.as_str(), upto = upto.get(), "session forked");
    Ok(json!({"session": child, "upto": upto.get()}))
}

/// 分叉不成写成拒绝：父会话不是主会话的 `bad_params`，日志没有的 `not_found`，别的记一行、内部出错。
fn refused(parent: &SessionId, error: &ForkError) -> Refusal {
    match error {
        ForkError::NotMain => Refusal::BAD_PARAMS,
        ForkError::Log(OpenError::Missing(_)) => Refusal::NOT_FOUND,
        _ => {
            tracing::warn!(target: TARGET, parent = parent.as_str(), error = %error, "session not forked");
            Refusal::INTERNAL
        }
    }
}
