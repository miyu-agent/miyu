//! 交接票（接入通讯平台设计 01 第九节第 3 条，核心的会话 2026-10-19 回的形状）：已经登录的主人要一张 60 秒、一次性的票，带在
//! 跳转的网址里；接的一方（im 的页面服务、`miyu web` 打开的网页）拿它换一个设备令牌，不用再登录一次。票只在内存里，最多 16 张，一个字都不进
//! 运行日志。
//!
//! - `login.ticket`：只给人（本机令牌、设备令牌、密码连上的），一次性码连上的、核心拉起的扩展回 `local_only`。回应 `{ticket, expires}`
//!   （施工 SU-1 起不带 `account`）。
//! - `login.redeem {ticket}`：谁拿着票谁能换（票是 32 个随机字节，猜不出来）；没有、过期、用过的回 `bad_ticket`。
//! - 握手的凭据 `ticket`（施工 LT-1，接入通讯平台设计 07 第四节 4.6 第 4 样）：待遇同密码，对上了当场作废，回应带新的设备令牌
//!   （[`handshake`]）；不认的同样是 `bad_ticket`。

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use super::{TARGET, Via, files, millis, random_hex, stamp};
use crate::Core;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 一张票多久有效。
pub(crate) const TICKET_TTL: Duration = Duration::from_secs(60);

/// 同时最多几张：多了丢最早的。
const MOST_TICKETS: usize = 16;

/// 一张还没用的票。
struct Ticket {
    value: String,
    expires: Instant,
}

/// 核心一份的票。
#[derive(Default)]
pub(crate) struct Tickets(Mutex<Vec<Ticket>>);

impl Tickets {
    /// 用掉 `value` 那一张：在、没过期的交回真，当场作废。
    fn take(&self, value: &str) -> bool {
        let mut tickets = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        tickets.retain(|each| each.expires > now);
        let Some(at) = tickets.iter().position(|each| each.value == value) else {
            return false;
        };
        tickets.remove(at);
        true
    }

    /// 记一张，`expires` 过期。
    fn put(&self, value: String, expires: Instant) {
        let mut tickets = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        tickets.retain(|each| each.expires > now);
        tickets.push(Ticket { value, expires });
        while tickets.len() > MOST_TICKETS {
            tickets.remove(0);
        }
    }
}

/// `login.redeem` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RedeemParams {
    ticket: String,
}

/// 答 `login.ticket`、`login.redeem`：`via` 是这个连接怎么认出来的。不认识的方法 `unknown_method`。
pub(crate) async fn call(
    core: &Core,
    via: Option<&Via>,
    request: &Request,
) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "login.ticket" => issue(core, via),
        "login.redeem" => {
            let params: RedeemParams =
                serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
            redeem(core, &params.ticket).await
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// `login.ticket`：交回 `{ticket, expires}`。
fn issue(core: &Core, via: Option<&Via>) -> Result<Value, Refusal> {
    if !matches!(via, Some(Via::Token | Via::Login(_) | Via::Password(_))) {
        return Err(Refusal::LOCAL_ONLY);
    }
    let ticket = random_hex()?;
    let until = Instant::now() + TICKET_TTL;
    core.identity.tickets.put(ticket.clone(), until);
    let expires = crate::sessions::now().unix_millis() + millis(TICKET_TTL);
    tracing::info!(target: TARGET, "ticket issued");
    Ok(json!({"ticket": ticket, "expires": stamp(expires)}))
}

/// `login.redeem`：票对得上的造一个设备令牌，交回 `{login: {token, expires}}`。
async fn redeem(core: &Core, ticket: &str) -> Result<Value, Refusal> {
    let (login, _) = exchange(core, ticket).await?;
    Ok(json!({"login": login}))
}

/// 握手的凭据 `ticket`（施工 LT-1）：票对得上的当场作废、造一个设备令牌，交回 `{token, expires}` 和这个连接怎么算（同密码连上的：
/// 带着新令牌的哈希，登出它的时候连接跟着断）。
pub(crate) async fn handshake(core: &Core, ticket: &str) -> Result<(Value, Via), Refusal> {
    let (login, hash) = exchange(core, ticket).await?;
    Ok((login, Via::Login(hash)))
}

/// 用掉票、造一个设备令牌：交回 `{token, expires}`、令牌的哈希。
async fn exchange(core: &Core, ticket: &str) -> Result<(Value, String), Refusal> {
    if !core.identity.tickets.take(ticket) {
        return Err(Refusal::BAD_TICKET);
    }
    let _queue = core.identity.files.lock().await;
    let (token, hash, expires) = files::issue(core, crate::sessions::now())?;
    tracing::info!(target: TARGET, via = "ticket", "login issued");
    let login = json!({"expires": expires, "token": token});
    Ok((login, hash))
}

#[cfg(test)]
mod tests;
