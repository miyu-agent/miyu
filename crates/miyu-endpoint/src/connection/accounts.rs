//! 连接上的 `account.*`（施工 W-8、Y-1 上）：设密码、退出登录、邀请码、注册、列出、停用、启用。设密码、注册换掉这个连接的
//! 凭据（[`Changed`]），连接照它改认的账号、收作废的广播。

use std::sync::Arc;

use serde_json::Value;

use miyu_kernel::id::AccountId;

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
    /// 凭据换成了这个（设了密码、注册成了）。
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

/// 办一条 `account.*`：`via` 是这个连接现在的凭据，`account` 是它现在是谁。
pub(super) async fn call(
    core: &Arc<Core>,
    via: Option<&Via>,
    account: &AccountId,
    request: &Request,
) -> Changed {
    let now = via.cloned().unwrap_or(Via::Token);
    let params = request.params.clone();
    match request.method.as_str() {
        "account.setup_code" => Changed::answer(login::setup_code(core, &now)),
        "account.setup" => match login::setup(core, &now, params).await {
            // 设置那一段注册了机主（施工 Y-12 中上）：答完就断，头重连进照机主起的那一份。
            Ok((result, by)) => Changed {
                result: Ok(result),
                close: core.setting_up(),
                via: Some(by),
            },
            Err(refusal) => Changed::answer(Err(refusal)),
        },
        "account.register" => match login::members::register(core, &now, params).await {
            Ok((result, by)) => Changed {
                result: Ok(result),
                close: false,
                via: Some(by),
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
        "account.invite" => Changed::answer(login::invites::invite(core, account, params).await),
        "account.invites" => Changed::answer(login::invites::list(core, account)),
        "account.invite_revoke" => {
            Changed::answer(login::invites::revoke(core, account, params).await)
        }
        "account.list" => Changed::answer(login::members::list(core, account)),
        "account.disable" => {
            Changed::answer(login::members::switch(core, account, params, true).await)
        }
        "account.enable" => {
            Changed::answer(login::members::switch(core, account, params, false).await)
        }
        _ => Changed::answer(Err(Refusal::UNKNOWN_METHOD)),
    }
}
