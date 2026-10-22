//! 登出、手贴回调（施工 X-42 下，`docs/blueprint/mcp.md`「怎么走」第八条）。
//!
//! - `mcp.logout {service}`（只给人）：在等回调的那一次登录一起取消，删掉 `mcp-oauth-<服务名>` 那一份（客户端、端口一起）；服务照
//!   密钥变了那一路重起、回 `401`、停在 `needs_login`。没登录过的 `not_logged_in`。
//! - `mcp.login_finish {service, url}`（只给人）：浏览器不在核心那台机器上的，把跳到的那一整段地址交给在等的那一次登录，和本机
//!   收到的回调同一处判（`flow.rs` 的 `Waiting::wait`）。对上了、换到令牌存好了回 `{}`；对不上的 `oauth_failed`，那一次登录照旧
//!   等着；没有在等的 `not_waiting`。

use std::sync::{Arc, PoisonError};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use miyu_kernel::id::CommandId;

use super::{Pasted, secret_name};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 运行日志的目标。
const TARGET: &str = "miyu::mcp";

/// `mcp.logout` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LogoutParams {
    /// 服务名。
    service: String,
}

/// `mcp.login_finish` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FinishParams {
    /// 服务名。
    service: String,
    /// 浏览器跳到的那一整段地址。
    url: String,
}

/// `mcp.logout`：交回 `{}`。
///
/// # Errors
///
/// 没登录过的 `not_logged_in`；密钥文件写不进的同 `secret.delete`。
pub(crate) fn logout(
    core: &Arc<Core>,
    peer: Peer,
    cause: &CommandId,
    params: LogoutParams,
) -> Result<Value, Refusal> {
    if let Some(waiting) = core
        .mcp
        .logins
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&params.service)
    {
        waiting.task.abort();
    }
    let name = secret_name(&params.service);
    if !core.config().secrets.has(&name) {
        return Err(Refusal::NOT_LOGGED_IN);
    }
    crate::secrets::remove(core, peer, cause, &name)?;
    tracing::info!(target: TARGET, service = params.service.as_str(), "mcp oauth logged out");
    Ok(json!({}))
}

/// `mcp.login_finish`：交回 `{}`。
///
/// # Errors
///
/// 没有在等的 `not_waiting`；对不上的、换不到令牌的 `oauth_failed`。
pub(crate) async fn finish(core: &Arc<Core>, params: FinishParams) -> Result<Value, Refusal> {
    let (answer, answered) = oneshot::channel();
    let pasted = Pasted {
        url: params.url,
        answer,
    };
    let sent = core
        .mcp
        .logins
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&params.service)
        .is_some_and(|waiting| waiting.pasted.send(pasted).is_ok());
    if !sent {
        return Err(Refusal::NOT_WAITING);
    }
    answered.await.unwrap_or(Err(Refusal::NOT_WAITING))?;
    Ok(json!({}))
}
