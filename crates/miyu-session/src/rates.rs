//! 取汇率（施工 UC-1，`docs/blueprint/models.md`「怎么走」第九条第 8 条）：用量汇总的取汇率端口（[`RateSource`]）的 HTTP 实现，
//! 核心起来时装上。主源 Frankfurter（`/v2/rates?base=USD&quotes=<币种>&from=<日>&to=<日>`，一个币种缺的几天并成一段一次取），
//! 主源连不上、出错的，当天和昨天的照兜底源 open.er-api.com 的最新一份（只有最新的，更早的日子不用它）。源说不认识的币种
//! （422）记成没有汇率。只交币种和日子，不交任何用量。走核心拉目录用的那个客户端（照环境的代理），5 秒、64 KiB 封顶。

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use serde_json::Value;
use tokio::runtime::Handle;

use miyu_http::{Client, Get, Got, get_full};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_store::usage::rates::{Day, Fetched, RateSource, Source};

use crate::TARGET;

/// Frankfurter 的地址。
pub const FRANKFURTER: &str = "https://api.frankfurter.dev";

/// open.er-api.com 的地址。
pub const FALLBACK: &str = "https://open.er-api.com";

/// 一次请求最多多久。
const TIMEOUT: Duration = Duration::from_secs(5);

/// 一次回应最多多大。
const LIMIT: usize = 64 * 1024;

/// 一天，毫秒。
const DAY_MS: i64 = 86_400_000;

/// 取汇率的：HTTP 客户端、两个源的地址（测试换成假的）、在哪个运行时上取。
#[derive(Clone)]
pub struct HttpRates {
    client: Client,
    frankfurter: String,
    fallback: String,
    handle: Handle,
}

impl HttpRates {
    /// 照客户端 `client` 在运行时 `handle` 上取，地址是出厂的两个。
    pub fn new(client: Client, handle: Handle) -> HttpRates {
        HttpRates::at(client, handle, FRANKFURTER, FALLBACK)
    }

    /// 同 [`HttpRates::new`]，两个源的地址照给的（测试用假服务器）。
    pub fn at(client: Client, handle: Handle, frankfurter: &str, fallback: &str) -> HttpRates {
        HttpRates {
            client,
            frankfurter: frankfurter.trim_end_matches('/').to_string(),
            fallback: fallback.trim_end_matches('/').to_string(),
            handle,
        }
    }

    /// 取 `wanted` 里的：照币种分，一个币种一次。
    async fn fetch(&self, wanted: Vec<(Day, String)>, today: Day) -> Vec<Fetched> {
        let mut by_currency: BTreeMap<String, BTreeSet<Day>> = BTreeMap::new();
        for (day, currency) in wanted {
            by_currency.entry(currency).or_default().insert(day);
        }
        let mut fetched = Vec::new();
        let mut latest: Option<Option<BTreeMap<String, f64>>> = None;
        for (currency, days) in by_currency {
            match self.frankfurter(&currency, &days).await {
                Ok(Some(rates)) => fetched.extend(settle(&currency, &days, &rates, today)),
                Ok(None) => fetched.extend(days.iter().map(|day| Fetched {
                    day: *day,
                    currency: currency.clone(),
                    rate: None,
                    source: Source::Frankfurter,
                    settled: false,
                })),
                Err(error) => {
                    tracing::warn!(target: TARGET, currency = currency.as_str(), error = error.as_str(), "rates not fetched");
                    // 兜底源只有最新的：只给当天、昨天。
                    let recent: Vec<Day> = days
                        .iter()
                        .copied()
                        .filter(|day| *day >= today - 1)
                        .collect();
                    if recent.is_empty() {
                        continue;
                    }
                    if latest.is_none() {
                        latest = Some(self.fallback().await);
                    }
                    if let Some(Some(rates)) = &latest
                        && let Some(rate) = rates.get(&currency)
                    {
                        fetched.extend(recent.into_iter().map(|day| Fetched {
                            day,
                            currency: currency.clone(),
                            rate: Some(*rate),
                            source: Source::Fallback,
                            settled: false,
                        }));
                    }
                }
            }
        }
        fetched
    }

    /// 主源：`currency` 在 `days` 这一段每天 1 美元换多少（照交回的日子）。源说不认识这个币种的是 `Ok(None)`。
    async fn frankfurter(
        &self,
        currency: &str,
        days: &BTreeSet<Day>,
    ) -> Result<Option<BTreeMap<Day, f64>>, String> {
        let (Some(first), Some(last)) = (days.first(), days.last()) else {
            return Ok(Some(BTreeMap::new()));
        };
        let url = format!(
            "{}/v2/rates?base=USD&quotes={currency}&from={}&to={}",
            self.frankfurter,
            date(*first),
            date(*last)
        );
        let body = match self.get(&url).await {
            Ok(body) => body,
            Err((Some(422), _)) => return Ok(None),
            Err((_, error)) => return Err(error),
        };
        let rows: Value =
            serde_json::from_slice(&body).map_err(|error| format!("answer not read: {error}"))?;
        let rows = rows.as_array().ok_or("answer is not a list")?;
        Ok(Some(
            rows.iter()
                .filter(|row| row["quote"].as_str() == Some(currency))
                .filter_map(|row| Some((day(row["date"].as_str()?)?, row["rate"].as_f64()?)))
                .collect(),
        ))
    }

    /// 兜底源的最新一份：币种到 1 美元换多少。取不到的没有。
    async fn fallback(&self) -> Option<BTreeMap<String, f64>> {
        let url = format!("{}/v6/latest/USD", self.fallback);
        match self.get(&url).await {
            Ok(body) => {
                let latest: Value = serde_json::from_slice(&body).ok()?;
                if latest["result"].as_str() != Some("success") {
                    return None;
                }
                Some(
                    latest["rates"]
                        .as_object()?
                        .iter()
                        .filter_map(|(currency, rate)| Some((currency.clone(), rate.as_f64()?)))
                        .collect(),
                )
            }
            Err((_, error)) => {
                tracing::warn!(target: TARGET, error = error.as_str(), "fallback rates not fetched");
                None
            }
        }
    }

    /// GET 一次：交回响应体，或者状态码（没有的没有）和原话。
    async fn get(&self, url: &str) -> Result<Vec<u8>, (Option<u16>, String)> {
        let got = get_full(Get {
            client: &self.client,
            url,
            headers: &[],
            etag: None,
            timeout: TIMEOUT,
            limit: LIMIT,
        })
        .await;
        match got {
            Ok(Got::Body { bytes, .. }) => Ok(bytes),
            Ok(Got::NotModified) => Err((Some(304), "not modified".to_string())),
            Err(failed) => Err((failed.status, failed.message)),
        }
    }
}

impl RateSource for HttpRates {
    fn start(
        &self,
        wanted: Vec<(Day, String)>,
        today: Day,
        done: Box<dyn FnOnce(Vec<Fetched>) + Send>,
    ) {
        let rates = self.clone();
        self.handle.spawn(async move {
            done(rates.fetch(wanted, today).await);
        });
    }
}

/// 照主源交回的 `rates`（日子到汇率）定 `days` 里每一天的：用那一天或者更早最近的一天的；正好是那一天、那一天又已经过完
/// 了的定了，别的暂定。一天都对不上的不交。
fn settle(
    currency: &str,
    days: &BTreeSet<Day>,
    rates: &BTreeMap<Day, f64>,
    today: Day,
) -> Vec<Fetched> {
    days.iter()
        .filter_map(|day| {
            let (found, rate) = rates.range(..=*day).next_back()?;
            Some(Fetched {
                day: *day,
                currency: currency.to_string(),
                rate: Some(*rate),
                source: Source::Frankfurter,
                settled: found == day && *day < today,
            })
        })
        .collect()
}

/// 写成 `2026-10-24`。
fn date(day: Day) -> String {
    Timestamp::from_unix_millis(day * DAY_MS)
        .map(|at| at.local_date(UtcOffset::UTC))
        .unwrap_or_default()
}

/// 读 `2026-10-24`；写法不对的没有。
fn day(text: &str) -> Option<Day> {
    let at = Timestamp::parse(&format!("{text}T00:00:00.000Z")).ok()?;
    Some(at.unix_millis().div_euclid(DAY_MS))
}

#[cfg(test)]
mod tests;
