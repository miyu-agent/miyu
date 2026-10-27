//! 交接票（接入通讯平台设计 01 第九节第 3 条，核心的会话 2026-10-19 回的形状）：已经登录的人要一张 60 秒、一次性、绑定这个账号的
//! 票，带在跳转的网址里；接的一方（im 的页面服务）拿它换一个登录令牌，人不用再登录一次。票只在内存里，最多 16 张，一个字都不进
//! 运行日志。
//!
//! - `login.ticket`：只给人（本机令牌、登录令牌、密码连上的），一次性码连上的、核心拉起的扩展回 `local_only`。回应带票是谁的
//!   （`account`，施工 LT-1：网页的 `as` 照它）。
//! - `login.redeem {ticket}`：谁拿着票谁能换（票是 32 个随机字节，猜不出来）；没有、过期、用过的回 `bad_ticket`。
//! - 握手的凭据 `ticket`（施工 LT-1，接入通讯平台设计 07 第四节 4.6 第 4 样）：待遇同密码，对上了当场作废，回应带新的登录令牌，
//!   连接就是票的那个账号（[`handshake`]）；不认的同样是 `bad_ticket`。

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;

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
    account: AccountId,
    expires: Instant,
}

/// 核心一份的票。
#[derive(Default)]
pub(crate) struct Tickets(Mutex<Vec<Ticket>>);

impl Tickets {
    /// 用掉 `value` 那一张：在、没过期的交回它绑的账号，当场作废。
    fn take(&self, value: &str) -> Option<AccountId> {
        let mut tickets = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        tickets.retain(|each| each.expires > now);
        let at = tickets.iter().position(|each| each.value == value)?;
        Some(tickets.remove(at).account)
    }

    /// 记一张绑 `account` 的，`expires` 过期。
    fn put(&self, value: String, account: AccountId, expires: Instant) {
        let mut tickets = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        tickets.retain(|each| each.expires > now);
        tickets.push(Ticket {
            value,
            account,
            expires,
        });
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

/// 答 `login.ticket`、`login.redeem`：`via` 是这个连接怎么认出来的，`account` 是它的账号。不认识的方法 `unknown_method`。
pub(crate) async fn call(
    core: &Core,
    via: Option<&Via>,
    account: &AccountId,
    request: &Request,
) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "login.ticket" => issue(core, via, account),
        "login.redeem" => {
            let params: RedeemParams =
                serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
            redeem(core, &params.ticket).await
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// `login.ticket`：交回 `{ticket, expires}`。
fn issue(core: &Core, via: Option<&Via>, account: &AccountId) -> Result<Value, Refusal> {
    if !matches!(via, Some(Via::Token | Via::Login(..) | Via::Password(..))) {
        return Err(Refusal::LOCAL_ONLY);
    }
    let ticket = random_hex()?;
    let until = Instant::now() + TICKET_TTL;
    core.identity
        .tickets
        .put(ticket.clone(), account.clone(), until);
    let expires = crate::sessions::now().unix_millis() + millis(TICKET_TTL);
    tracing::info!(target: TARGET, "ticket issued");
    Ok(json!({"ticket": ticket, "expires": stamp(expires), "account": account.as_str()}))
}

/// `login.redeem`：票对得上的造一个登录令牌，交回 `{account, login: {token, expires}}`。
async fn redeem(core: &Core, ticket: &str) -> Result<Value, Refusal> {
    let (account, login, _) = exchange(core, ticket).await?;
    Ok(json!({"account": account.as_str(), "login": login}))
}

/// 握手的凭据 `ticket`（施工 LT-1）：票对得上的当场作废、造一个登录令牌，交回 `{token, expires}` 和这个连接怎么算（同密码连上的：
/// 带着新令牌的哈希和票的账号，登出它的时候连接跟着断）。
pub(crate) async fn handshake(core: &Core, ticket: &str) -> Result<(Value, Via), Refusal> {
    let (account, login, hash) = exchange(core, ticket).await?;
    Ok((login, Via::Login(hash, account)))
}

/// 用掉票、给它的账号造一个登录令牌：交回账号、`{token, expires}`、令牌的哈希。
async fn exchange(core: &Core, ticket: &str) -> Result<(AccountId, Value, String), Refusal> {
    let account = core
        .identity
        .tickets
        .take(ticket)
        .ok_or(Refusal::BAD_TICKET)?;
    let _queue = core.identity.files.lock().await;
    let (token, hash, expires) = files::issue(core, &account, crate::sessions::now())?;
    tracing::info!(target: TARGET, via = "ticket", "login issued");
    let login = json!({"expires": expires, "token": token});
    Ok((account, login, hash))
}

#[cfg(test)]
mod tests;
