//! 连接上的 `account.*`（施工 W-8）：设访问密码、退出登录、头像。一次性码连上的设了访问密码换掉这个
//! 连接的凭据（[`Changed`]），连接照它收作废的广播。

use std::sync::Arc;

use serde_json::Value;

use crate::Core;
use crate::login::{self, Via};
use crate::refusal::Refusal;
use crate::wire::Request;

/// 办完一条 `account.*`：回应、这个连接要不要断开、凭据换成了什么。
pub(super) struct Changed {
    /// 回应。
    pub(super) result: Result<Value, Refusal>,
    /// 回完断开。
    pub(super) close: bool,
    /// 凭据换成了这个（一次性码连上的设了访问密码）。
    pub(super) via: Option<Via>,
}

impl Changed {
    /// 只有回应。
    fn answer(result: Result<Value, Refusal>) -> Changed {
        Changed {
            result,
            close: false,
            via: None,
        }
    }
}

/// 办一条 `account.*`：`via` 是这个连接现在的凭据，`source` 是它的来源（网页软件转来的地址；本机来的没有）。
pub(super) async fn call(
    core: &Arc<Core>,
    (via, source): (Option<&Via>, Option<&str>),
    request: &Request,
) -> Changed {
    let now = via.cloned().unwrap_or(Via::Token);
    let params = request.params.clone();
    match request.method.as_str() {
        "account.setup_code" => Changed::answer(login::setup_code(core, &now)),
        "account.setup" => match login::setup(core, (&now, source), params).await {
            Ok((result, by)) => Changed {
                result: Ok(result),
                close: false,
                via: by,
            },
            Err(refusal) => Changed::answer(Err(refusal)),
        },
        "account.logout" => match login::logout(core, &now, params).await {
            Ok((result, close)) => Changed {
                result: Ok(result),
                close,
                via: None,
            },
            Err(refusal) => Changed::answer(Err(refusal)),
        },
        // 你的头像（施工 AV-1）。
        "account.set_avatar" => Changed::answer(match crate::methods::params(request) {
            Ok(params) => login::avatar::set(core, params).await,
            Err(refusal) => Err(refusal),
        }),
        "account.avatar" => Changed::answer(match crate::methods::params(request) {
            Ok(params) => login::avatar::read(core, params).await,
            Err(refusal) => Err(refusal),
        }),
        _ => Changed::answer(Err(Refusal::UNKNOWN_METHOD)),
    }
}
