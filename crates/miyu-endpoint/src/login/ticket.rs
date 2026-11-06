//! 交接票（接入通讯平台设计 01 第九节第 3 条，核心的会话 2026-10-19 回的形状）：已经登录的主人要一张 60 秒、一次性的票，带在
//! 跳转的网址里；接的一方（im 的页面服务、`miyu web` 打开的网页）拿它换一个设备令牌，不用再登录一次。票只在内存里，最多 16 张，一个字都不进
//! 运行日志。
//!
//! - `login.ticket`：给人（本机令牌、设备令牌、密码连上的），一次性码连上的回 `local_only`。回应 `{ticket, expires}`。
//!   参数可以写 `via`（施工 LT-2）：`page` 是网页服务替本机页面要的（本机打开网页不要凭据，网页 W-1089），`open` 是 `miyu web`
//!   开网页要的；进运行日志（连同这个连接怎么认出来的），不进回应。`page` 的票不管谁要的都记成「只在本机换」（人要的施工 LP-2，
//!   扩展要的 LP-1，下面）；别的、不写的哪儿都能换。别的值、别的格 `bad_params`。
//! - 核心拉起的扩展（施工 LP-1，项目主人 11-06 定只给本机免密）：只能要 `via` 是 `page` 的，要批过能力 `login.page`（出厂的也要人批），
//!   没批的 `needs_approval`；别的 `via`、不写的照旧 `local_only`（都在 `connection/rights.rs` 挡）。它要的票记成「只在本机换」：
//!   远程的连接（网页软件说过 `connection.remote` 的，不管说的是什么地址）拿它握手、`login.redeem` 都回 `bad_ticket`，票当场作废。
//!   扩展只该给核过是本机来的页面要票（参考 `extensions.md`），核心这一道挡的是票被带到远程的连接上换。
//! - 人要的 `page` 票（施工 LP-2）：网页软件 `/ticket` 出示本机令牌替本机页面要的，页面在本机的 `/ws` 上握手用掉；一样只在本机换，
//!   挡法同上。`open`（`miyu web --print` 印出来交给别的程序）、不写的（网页给 im 的页面开新标签页）照旧哪儿都能换。
//! - 每发一张记一行 `INFO ticket issued`：`purpose` 是 `via`，`by` 是这个连接怎么认出来的，扩展要的另记 `package`；不记票。
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
    /// 只在本机换（`page` 的票：扩展要的施工 LP-1，人要的 LP-2）。
    here: bool,
}

/// 一张票是只在本机换的，还是哪儿都能换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// 哪儿都能换：人要的 `open`、不写 `via` 的。
    Anywhere,
    /// 只在本机的连接上换：`page` 的，扩展（施工 LP-1）、人（LP-2）替本机页面要的。
    Here,
}

/// 核心一份的票。
#[derive(Default)]
pub(crate) struct Tickets(Mutex<Vec<Ticket>>);

impl Tickets {
    /// 用掉 `value` 那一张：在、没过期的当场作废，交回它在哪儿能换；没有的交回没有。
    fn take(&self, value: &str) -> Option<Reach> {
        let mut tickets = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        tickets.retain(|each| each.expires > now);
        let at = tickets.iter().position(|each| each.value == value)?;
        let taken = tickets.remove(at);
        Some(if taken.here {
            Reach::Here
        } else {
            Reach::Anywhere
        })
    }

    /// 记一张，`expires` 过期，`reach` 是在哪儿能换。
    fn put(&self, value: String, expires: Instant, reach: Reach) {
        let mut tickets = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        tickets.retain(|each| each.expires > now);
        let here = reach == Reach::Here;
        tickets.push(Ticket {
            value,
            expires,
            here,
        });
        while tickets.len() > MOST_TICKETS {
            tickets.remove(0);
        }
    }
}

/// `login.ticket` 的参数（施工 LT-2）：都可以不写。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IssueParams {
    via: Option<Purpose>,
}

/// 票是给谁要的：进运行日志；`page` 的只在本机换（[`Purpose::reach`]）。
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Purpose {
    /// 网页服务替本机打开的页面要的。
    Page,
    /// `miyu web` 开网页要的。
    Open,
}

impl Purpose {
    fn as_str(self) -> &'static str {
        match self {
            Purpose::Page => "page",
            Purpose::Open => "open",
        }
    }

    /// 要的票在哪儿能换：`page` 的只在本机（不管是人还是扩展要的，施工 LP-1、LP-2），别的、不写的哪儿都能。
    fn reach(purpose: Option<Self>) -> Reach {
        match purpose {
            Some(Purpose::Page) => Reach::Here,
            Some(Purpose::Open) | None => Reach::Anywhere,
        }
    }
}

/// `login.redeem` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RedeemParams {
    ticket: String,
}

/// 问票、换票的这个连接：怎么认出来的（`via`）、核心拉起的扩展是哪个包（`package`，本机的头没有）、是不是本机的（`here`：没说过
/// `connection.remote` 的）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Asker<'a> {
    pub(crate) via: Option<&'a Via>,
    pub(crate) package: Option<&'a str>,
    pub(crate) here: bool,
}

/// 答 `login.ticket`、`login.redeem`。不认识的方法 `unknown_method`。
pub(crate) async fn call(
    core: &Core,
    asker: Asker<'_>,
    request: &Request,
) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "login.ticket" => {
            let params: IssueParams =
                serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
            issue(core, asker, params.via)
        }
        "login.redeem" => {
            let params: RedeemParams =
                serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
            redeem(core, &params.ticket, asker.here).await
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// `login.ticket`：交回 `{ticket, expires}`。人什么 `via` 都能要；扩展只能要 `page` 的（能力 `login.page` 在 `connection/rights.rs`
/// 挡过了）。`page` 的票只在本机换（施工 LP-1、LP-2），别的哪儿都能换（[`Purpose::reach`]）。运行日志记一行：给谁要的
/// （`purpose`）、这个连接怎么认出来的（`by`），扩展的另记包编号，不记票。
fn issue(core: &Core, asker: Asker<'_>, purpose: Option<Purpose>) -> Result<Value, Refusal> {
    let by = match (asker.via, purpose) {
        (Some(Via::Token), _) => "token",
        (Some(Via::Login(_)), _) => "login",
        (Some(Via::Password(_)), _) => "password",
        (Some(Via::Spawned), Some(Purpose::Page)) => "extension",
        _ => return Err(Refusal::LOCAL_ONLY),
    };
    let reach = Purpose::reach(purpose);
    let ticket = random_hex()?;
    let until = Instant::now() + TICKET_TTL;
    core.identity.tickets.put(ticket.clone(), until, reach);
    let expires = crate::sessions::now().unix_millis() + millis(TICKET_TTL);
    let purpose = purpose.map_or("unknown", Purpose::as_str);
    if by == "extension" {
        let package = asker.package.unwrap_or_default();
        tracing::info!(target: TARGET, purpose, by, package, "ticket issued");
    } else {
        tracing::info!(target: TARGET, purpose, by, "ticket issued");
    }
    Ok(json!({"ticket": ticket, "expires": stamp(expires)}))
}

/// `login.redeem`：票对得上的造一个设备令牌，交回 `{login: {token, expires}}`。`here` 是这个连接是不是本机的。
async fn redeem(core: &Core, ticket: &str, here: bool) -> Result<Value, Refusal> {
    let (login, _) = exchange(core, ticket, here).await?;
    Ok(json!({"login": login}))
}

/// 握手的凭据 `ticket`（施工 LT-1）：票对得上的当场作废、造一个设备令牌，交回 `{token, expires}` 和这个连接怎么算（同密码连上的：
/// 带着新令牌的哈希，登出它的时候连接跟着断）。`here` 是这个连接是不是本机的（没说过 `connection.remote`）。
pub(crate) async fn handshake(
    core: &Core,
    ticket: &str,
    here: bool,
) -> Result<(Value, Via), Refusal> {
    let (login, hash) = exchange(core, ticket, here).await?;
    Ok((login, Via::Login(hash)))
}

/// 用掉票、造一个设备令牌：交回 `{token, expires}`、令牌的哈希。只在本机换的票（`page` 的，施工 LP-1、LP-2）到了不是本机的连接（`here` 是假的）上，
/// 照不认得的回 `bad_ticket`，票已经作废了，记一行 `WARN`（不记票）。
async fn exchange(core: &Core, ticket: &str, here: bool) -> Result<(Value, String), Refusal> {
    match core.identity.tickets.take(ticket) {
        None => return Err(Refusal::BAD_TICKET),
        Some(Reach::Here) if !here => {
            tracing::warn!(target: TARGET, "ticket for this machine refused off it");
            return Err(Refusal::BAD_TICKET);
        }
        Some(_) => {}
    }
    let _queue = core.identity.files.lock().await;
    let (token, hash, expires) = files::issue(core, crate::sessions::now())?;
    tracing::info!(target: TARGET, via = "ticket", "login issued");
    let login = json!({"expires": expires, "token": token});
    Ok((login, hash))
}

#[cfg(test)]
mod tests;
