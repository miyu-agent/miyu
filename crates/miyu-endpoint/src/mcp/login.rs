//! 网址的服务登录（施工 X-42 上，`docs/blueprint/mcp.md`「怎么走」第八条）：服务回 `401` 要登录、这张表又没写令牌的，看管的一头
//! 停在 `needs_login`（`Reason::NeedsLogin`），挑战记在看管那一格；人经 `mcp.login` 登录（`flow.rs`），令牌存进密钥文件的
//! `mcp-oauth-<服务名>`（一段 JSON），连的时候当 `bearer` 带上。密钥变了照 X-32 补那一路重起服务。
//!
//! 这张表写了 `bearer`、或者 `headers` 里写了 `Authorization` 的是人自己给的令牌：不走登录，回 `401` 照旧当连不上。
//!
//! 施工 X-42 下：连的时候转接每个请求现场问令牌（`credentials.rs`），快过期的先续、回 `401` 的续一次，续期写回的那一份不重起服务；
//! 登出、浏览器不在核心那台机器上时把跳到的地址贴回来（`manage.rs`）。

use std::sync::PoisonError;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use serde::{Deserialize, Serialize};

use miyu_config::package::{McpServer, Reach};
use miyu_config::secret::Reference;
use miyu_mcp::oauth::Challenge;

use super::{Servers, Shared};
use crate::config::Config;
use crate::refusal::Refusal;

mod credentials;
mod flow;
mod manage;

pub(crate) use credentials::Login;
pub(crate) use flow::login;
pub(crate) use manage::{finish, logout};

/// 存令牌的密钥叫什么：`mcp-oauth-<服务名>`（服务名合密钥名字的写法）。
pub(crate) fn secret_name(service: &str) -> String {
    format!("mcp-oauth-{service}")
}

/// 存进密钥里的那一份：令牌、它从哪个授权服务器来、哪个客户端拿的、回调用的端口（照服务记住，下一次绑同一个）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Stored {
    /// `access_token`：连的时候当 `bearer`。
    pub(crate) access_token: String,
    /// `refresh_token`，没给的没有（续期是施工 X-42 下）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) refresh_token: Option<String>,
    /// 到期的那一刻（Unix 秒），没说的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) expires_at: Option<u64>,
    /// 实际给的范围。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scope: Option<String>,
    /// 授权服务器的 `issuer`：换了授权服务器的，客户端不再用。
    pub(crate) issuer: String,
    /// 客户端编号。
    pub(crate) client_id: String,
    /// 动态注册拿到的客户端密钥，没有的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) client_secret: Option<String>,
    /// 回调的端口：和 `client_id` 同进退（主会话 2026-10-21 定）。
    pub(crate) port: u16,
    /// 客户端是动态注册来的：端口被占时照新端口重新注册一次。
    #[serde(default)]
    pub(crate) registered: bool,
    /// 登录时的 `resource`（施工 X-42 下：续期带同一个）。X-42 上存的没有，照服务的规范网址。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) resource: Option<String>,
}

/// 这一刻（Unix 秒）。
pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// 一次在等回调的登录（施工 X-42 下）：等着的任务，和贴回来的地址交给它的那一头。
pub(crate) struct Ongoing {
    pub(super) task: JoinHandle<()>,
    pub(super) pasted: mpsc::UnboundedSender<Pasted>,
}

/// 贴回来的一段地址（`mcp.login_finish`），和判完的结局交回去的那一头。
pub(super) struct Pasted {
    pub(super) url: String,
    pub(super) answer: oneshot::Sender<Result<(), Refusal>>,
}

impl Servers {
    /// 记下续期写回去的那一份（施工 X-42 下）：密钥 `name` 的值 `value`，只留摘要。
    pub(super) fn wrote(&self, name: &str, value: &str) {
        self.wrote
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(name.to_string(), super::secrets::print(value));
    }

    /// 密钥 `name` 这一刻的摘要 `now` 是不是续期刚写回去的那一份：是的不算人改的、不重起。删掉了的不算。
    pub(super) fn wrote_now(&self, name: &str, now: Option<&u64>) -> bool {
        now.is_some()
            && self
                .wrote
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(name)
                == now
    }
}

/// 服务 `service` 存着的那一份：没有的、读不成的没有。
pub(crate) fn stored(config: &Config, service: &str) -> Option<Stored> {
    let secret = config.secret(&Reference::Secret(secret_name(service)))?;
    serde_json::from_str(secret.expose()).ok()
}

/// 这张表自己写了令牌：`bearer`，或者 `headers` 里的 `Authorization`（不分大小写）。
pub(crate) fn explicit(server: &McpServer) -> bool {
    match &server.reach {
        Reach::Url {
            bearer, headers, ..
        } => {
            bearer.is_some()
                || headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        }
        Reach::Command { .. } => false,
    }
}

/// 连不上时要不要登录：网址的服务、这张表没写令牌、对方回过 `401` 且是 `Bearer` 的挑战。
pub(super) fn needed(server: &McpServer, header: Option<String>) -> Option<Challenge> {
    if !matches!(server.reach, Reach::Url { .. }) || explicit(server) {
        return None;
    }
    miyu_mcp::oauth::challenge(&header?)
}

impl Shared {
    /// 记下（`Some`）、清掉（`None`）要登录时的挑战。
    pub(super) fn set_login(&self, challenge: Option<Challenge>) {
        *self.login.lock().unwrap_or_else(PoisonError::into_inner) = challenge;
    }

    /// 要登录时的挑战：没在等登录的没有。
    pub(super) fn login(&self) -> Option<Challenge> {
        self.login
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests;
