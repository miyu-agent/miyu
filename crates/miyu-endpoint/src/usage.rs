//! 用量汇总在协议这一头（施工 8-15，`docs/blueprint/models.md`「协议」`usage.query`、「怎么走」第九条第 6 条）：核心起来时开
//! `state/usage.db`（[`open`]），`usage.query` 先补再查（[`query`]）。
//!
//! - 参数：`from`、`until` 是时刻（左闭右开），`group` 是 `account`、`venue`、`model`、`day`、`session`、`purpose` 里的几样，
//!   `session` 只算这个会话、写了 `tree` 的连它派的子会话，`offset` 是分天的时区（`+09:00` 这样写，不写是核心所在机器此刻
//!   的）。不认识的分组、写法不对的时刻、时区、会话编号是 `bad_params`；不认识的格不理，「可以不写」的写 `null` 等于没写。
//! - 补：管理员的账号日志、会话、回收处里的会话；读不完的记一行 `WARN usage not indexed`，照读到的算。
//! - 回应 `rows`：分组的几格（没有的是 `null`），`requests`、`usage` 四项、`amounts`（照币种各加各的，`usage.currency` 排
//!   最前，别的照币种代码的字母先后，取的是这一刻的配置：当场生效）、`unpriced`。
//! - 读出坏了的：记一行 `WARN usage not read`，删掉重建再补一次；还不行的回 `internal`。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_kernel::event::Real;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_models::price::ordered;
use miyu_models::settings::UsageSettings;
use miyu_store::root::DataRoot;
use miyu_store::sqlite::Opened;
use miyu_store::usage::{Group, Query, Total, UsageError, UsageIndex};

use crate::Core;
use crate::refusal::Refusal;

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// 打开数据根 `root` 的用量汇总，记一行是什么情形：新建的 `INFO usage index created`，读不了、坏了、版本不对的删掉重建
/// `WARN usage index rebuilt reason=…`，重建也打不开的 `WARN usage index unusable error=…`（这一回不记账，查出来是空的）。
pub(crate) fn open(root: &DataRoot) -> UsageIndex {
    let (index, opened) = UsageIndex::open(root);
    match opened {
        Opened::Kept => {}
        Opened::Created => tracing::info!(target: TARGET, "usage index created"),
        Opened::Rebuilt(why) => {
            tracing::warn!(target: TARGET, reason = %why, "usage index rebuilt")
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, error = %error, "usage index unusable");
        }
    }
    index
}

/// `usage.query` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct QueryParams {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    until: Option<String>,
    #[serde(default)]
    group: Option<Vec<String>>,
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    tree: Option<bool>,
    #[serde(default)]
    offset: Option<String>,
}

/// `usage.query`：见模块的说明。不是管理员的人只算自己名下的（施工 Y-2 下下上）。
pub(crate) async fn query(
    core: &Arc<Core>,
    caller: &crate::hello::Caller,
    params: QueryParams,
) -> Result<Value, Refusal> {
    let mut query = read(params, crate::sessions::offset)?;
    let whose = caller.acting_for();
    if !crate::login::admins::is_admin(core, whose) {
        query.account = Some(whose.as_str().to_string());
    }
    let currency = currency(core, whose);
    let usage = Arc::clone(&core.usage);
    // 系统账号、成员的算进总数，照 `account` 能分开看（施工 O-4 下、Y-2 下下中下）。
    let accounts = core.usage_accounts();
    let group = query.group.clone();
    let totals = tokio::task::spawn_blocking(move || match caught_up(&usage, &accounts, &query) {
        Ok(totals) => Ok(totals),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "usage not read");
            if let Err(error) = usage.reset() {
                tracing::warn!(target: TARGET, error = %error, "usage index unusable");
            }
            caught_up(&usage, &accounts, &query)
        }
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
    .map_err(|error| {
        tracing::warn!(target: TARGET, error = %error, "usage not read");
        Refusal::INTERNAL
    })?;
    let rows: Vec<Value> = totals
        .into_iter()
        .map(|total| row(&group, total, &currency))
        .collect();
    Ok(json!({"rows": rows}))
}

/// 先补再查。
fn caught_up(
    usage: &UsageIndex,
    accounts: &[miyu_kernel::id::AccountId],
    query: &Query,
) -> Result<Vec<Total>, UsageError> {
    for problem in usage.catch_up(accounts)? {
        tracing::warn!(target: TARGET, session = problem.source.as_str(), error = problem.error.as_str(), "usage not indexed");
    }
    usage.query(query)
}

/// 读参数；`now_offset` 是不写时区时用的（核心所在机器此刻的）。
fn read(params: QueryParams, now_offset: fn() -> UtcOffset) -> Result<Query, Refusal> {
    let time = |text: Option<String>| {
        text.map(|text| Timestamp::parse(&text).map_err(|_| Refusal::BAD_PARAMS))
            .transpose()
    };
    let group = params
        .group
        .unwrap_or_default()
        .iter()
        .map(|name| Group::parse(name).ok_or(Refusal::BAD_PARAMS))
        .collect::<Result<Vec<_>, _>>()?;
    let session = params
        .session
        .map(|text| SessionId::parse(&text).map_err(|_| Refusal::BAD_PARAMS))
        .transpose()?
        .map(|id| (id, params.tree.unwrap_or(false)));
    let offset = match params.offset {
        Some(text) => offset(&text).ok_or(Refusal::BAD_PARAMS)?,
        None => now_offset(),
    };
    Ok(Query {
        from: time(params.from)?,
        until: time(params.until)?,
        group,
        session,
        account: None,
        offset,
    })
}

/// 时区：`+09:00`、`-05:30`，`±14:00` 以内；别的写法没有。
fn offset(text: &str) -> Option<UtcOffset> {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| -> Option<i32> {
        let part = text.get(range)?;
        part.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    if bytes.len() != 6 || bytes[3] != b':' {
        return None;
    }
    let sign = match bytes[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let (hours, minutes) = (digits(1..3)?, digits(4..6)?);
    if minutes >= 60 {
        return None;
    }
    UtcOffset::from_minutes(sign * (hours * 60 + minutes))
}

/// 一行写成协议上的样子：分组的几格照 `group` 的先后，再是请求数、用量、金额（`first` 排最前）、没金额的次数。
fn row(group: &[Group], total: Total, first: &str) -> Value {
    let mut map = Map::new();
    for (group, key) in group.iter().zip(total.keys) {
        map.insert(group.name().to_string(), json!(key));
    }
    let mut amounts = total.amounts;
    ordered(&mut amounts, first);
    let amounts: Vec<Value> = amounts
        .into_iter()
        .map(|(currency, amount)| json!({"currency": currency, "amount": Real::new(amount)}))
        .collect();
    map.insert("requests".to_string(), json!(total.requests));
    map.insert("usage".to_string(), json!(total.usage));
    map.insert("amounts".to_string(), Value::Array(amounts));
    map.insert("unpriced".to_string(), json!(total.unpriced));
    Value::Object(map)
}

/// 订阅的回应里这个会话累计的（施工 9-6 上，`protocol.md` 的 `subscribe`）：和 `usage.query {"session": <它>}` 那一行同一个
/// 写法、同一个口径（金额照这一刻的 `usage.currency` 排），另加只算主请求的用量（`main`，施工 9-6 上补）、压缩过几次、缓存断了几次。
/// 会话 `session` 那一支（连它派的子会话）一共用了多少（施工 9-8 补下）：先补再查，在阻塞线程里。读不了的是空的。
/// 会话 `session` 和它派的子会话（施工 CX-3，`session.usage_parts` 的 `tree`）：先补再查，同 `usage.query` 的 `tree`。读不了的
/// 记一行、只有它自己。
pub(crate) async fn tree(core: &Core, session: &SessionId) -> Vec<SessionId> {
    let usage = Arc::clone(&core.usage);
    let accounts = core.usage_accounts();
    let root = session.as_str().to_string();
    let found = tokio::task::spawn_blocking(move || {
        for problem in usage.catch_up(&accounts)? {
            tracing::warn!(target: TARGET, session = problem.source.as_str(), error = problem.error.as_str(), "usage not indexed");
        }
        usage.tree(&root)
    })
    .await;
    match found {
        Ok(Ok(found)) => found
            .iter()
            .filter_map(|id| SessionId::parse(id).ok())
            .collect(),
        Ok(Err(error)) => {
            tracing::warn!(target: TARGET, error = %error, "usage not read");
            vec![session.clone()]
        }
        Err(_) => vec![session.clone()],
    }
}

pub(crate) async fn branch(core: &Core, session: &SessionId) -> Option<Total> {
    let usage = Arc::clone(&core.usage);
    let accounts = core.usage_accounts();
    let query = Query {
        from: None,
        until: None,
        group: Vec::new(),
        session: Some((session.clone(), true)),
        account: None,
        offset: crate::sessions::offset(),
    };
    let totals = tokio::task::spawn_blocking(move || caught_up(&usage, &accounts, &query))
        .await
        .ok()?;
    match totals {
        Ok(totals) => Some(totals.into_iter().next().unwrap_or_else(empty)),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "usage not read");
            None
        }
    }
}

/// 一行都没有时的总计：全是零。
fn empty() -> Total {
    Total {
        keys: Vec::new(),
        requests: 0,
        usage: miyu_kernel::event::Usage {
            uncached: 0,
            cache_read: 0,
            cache_write: 0,
            output: 0,
            reasoning: None,
        },
        amounts: Vec::new(),
        unpriced: 0,
    }
}

/// 账号 `account` 的金额哪个币种排最前：照他这一刻的那一份配置（施工 Y-2 下下中下）。
pub(crate) fn currency(core: &Core, account: &AccountId) -> String {
    UsageSettings::from(&core.mine(account).config().resolved().values()).currency
}

/// 写成一行（施工 9-8 补下）：写法同 `usage.query` 的一行，币种照会话属主 `owner` 这一刻的配置排。
pub(crate) fn total_row(core: &Core, owner: &AccountId, total: Total) -> Value {
    row(&[], total, &currency(core, owner))
}

/// 两份加起来：请求数、四项、金额（同一币种相加）、没价格的次数。
pub(crate) fn plus(mut one: Total, other: &Total) -> Total {
    one.requests += other.requests;
    one.usage = one.usage.plus(&other.usage);
    one.unpriced += other.unpriced;
    for (currency, amount) in &other.amounts {
        match one.amounts.iter_mut().find(|(have, _)| have == currency) {
            Some((_, sum)) => *sum += amount,
            None => one.amounts.push((currency.clone(), *amount)),
        }
    }
    one.amounts.sort_by(|a, b| a.0.cmp(&b.0));
    one
}

/// 一个会话自己累计的写成 [`Total`]。
pub(crate) fn own(tally: &miyu_session::Tally) -> Total {
    Total {
        keys: Vec::new(),
        requests: tally.requests,
        usage: tally.usage,
        amounts: tally
            .amounts
            .iter()
            .map(|(currency, amount)| (currency.clone(), *amount))
            .collect(),
        unpriced: tally.unpriced,
    }
}

/// 会话自己累计的写成协议上的一行，币种照会话属主 `owner` 这一刻的配置排（施工 Y-2 下下中下）。
pub(crate) fn tallied(core: &Core, owner: &AccountId, tally: &miyu_session::Tally) -> Value {
    let currency = currency(core, owner);
    let total = Total {
        keys: Vec::new(),
        requests: tally.requests,
        usage: tally.usage,
        amounts: tally
            .amounts
            .iter()
            .map(|(currency, amount)| (currency.clone(), *amount))
            .collect(),
        unpriced: tally.unpriced,
    };
    let mut value = row(&[], total, &currency);
    value["main"] = json!(tally.main);
    value["compactions"] = json!(tally.compactions);
    value["cache_breaks"] = json!(tally.cache_breaks);
    value
}

#[cfg(test)]
mod tests;
