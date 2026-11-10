//! `usage.convert`（施工 UC-3，`docs/blueprint/models.md`「协议」`usage.convert`）：扩展自己照日子、币种记的金额交给核心换算。
//!
//! - 参数 `groups`：一组是要合成一个数的那几笔，一笔是 `day`（UTC 日子 `2026-11-10`）、`currency`（三个字母的币种代码）、
//!   `amount`（数）。最多 [`MAX_GROUPS`] 组、合计最多 [`MAX_ENTRIES`] 笔；写错的、超了的都是 `bad_params`。
//! - 回应 `currency`（这一刻的 `usage.currency`）、`converted`：照组的先后一组一项，写法同 `usage.query` 一行里的
//!   `converted`（同一个函数写的），一笔都没换成的（连同空的组）是 `null`。
//! - 汇率照 `usage.query` 的等法：几组一起只找一次，缺的当场去取、最多等 [`WAIT`]。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::time::Timestamp;
use miyu_store::usage::Daily;
use miyu_store::usage::rates::{Day, WAIT, day_of};

use crate::Core;
use crate::refusal::Refusal;

/// 一次最多几组。
const MAX_GROUPS: usize = 1000;

/// 一次几组合起来最多几笔。
const MAX_ENTRIES: usize = 20_000;

/// `usage.convert` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct ConvertParams {
    groups: Vec<Vec<Entry>>,
}

/// 一笔：哪个 UTC 日子、什么币种、多少。
#[derive(Debug, Deserialize)]
struct Entry {
    day: String,
    currency: String,
    amount: f64,
}

/// `usage.convert`：见模块的说明。
pub(crate) async fn convert(core: &Arc<Core>, params: ConvertParams) -> Result<Value, Refusal> {
    let groups = read(params)?;
    let currency = super::currency(core);
    let usage = Arc::clone(&core.usage);
    let wanted = currency.clone();
    let converted = tokio::task::spawn_blocking(move || {
        let groups: Vec<&[Daily]> = groups.iter().map(Vec::as_slice).collect();
        usage.convert_all(&groups, &wanted, crate::sessions::now(), WAIT)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let converted: Vec<Value> = converted
        .into_iter()
        .map(|converted| converted.map_or(Value::Null, super::converted_value))
        .collect();
    Ok(json!({"currency": currency, "converted": converted}))
}

/// 读参数：超上限的、日子或币种写法不对的是 `bad_params`。扩展没有分四种的账，交出去的不带。
fn read(params: ConvertParams) -> Result<Vec<Vec<Daily>>, Refusal> {
    let entries: usize = params.groups.iter().map(Vec::len).sum();
    if params.groups.len() > MAX_GROUPS || entries > MAX_ENTRIES {
        return Err(Refusal::BAD_PARAMS);
    }
    params
        .groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|entry| {
                    let day = day(&entry.day).ok_or(Refusal::BAD_PARAMS)?;
                    if !code(&entry.currency) {
                        return Err(Refusal::BAD_PARAMS);
                    }
                    Ok(Daily {
                        day,
                        currency: entry.currency,
                        amount: entry.amount,
                        parts: None,
                    })
                })
                .collect()
        })
        .collect()
}

/// 读 `2026-11-10`；写法不对、没有这一天的没有。
fn day(text: &str) -> Option<Day> {
    let at = Timestamp::parse(&format!("{text}T00:00:00.000Z")).ok()?;
    Some(day_of(at.unix_millis()))
}

/// 是不是币种代码的样子：三个英文字母。币种会原样拼进问汇率源的网址，别的字不放进去。
fn code(text: &str) -> bool {
    text.len() == 3 && text.bytes().all(|byte| byte.is_ascii_alphabetic())
}
