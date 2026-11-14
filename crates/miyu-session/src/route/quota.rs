//! 查供应商的额度（施工 QU-1，`docs/blueprint/models.md`「怎么走」第十七条）：档案写了 `quota` 的一家（[`miyu_models::quota`]），
//! GET 那个地址，读成几个窗口。
//!
//! - 谁来查：协议的 `session.quota` 照会话接下来请求的那一家查；会话每轮结束在后台查一次（[`refresh`]，不挡回合）。
//! - 同一家 [`REUSE`] 内用上一次的结果，查不到的也算一次：几个会话同时收尾、头接着来问，只发一次。同一时刻只有一个在查，后到的
//!   等它查完照它答。只在这个进程的内存里，核心重启就清零。
//! - 头：`Authorization: Bearer <这一家的 key>`，档案另配的头（`x-opencode-session`）照固定的种子 [`SEED`] 换；
//!   `User-Agent` 是客户端的 `miyu/<版本>`（Cloudflare 挡默认库的）。地址落在本机的不走代理，同拉列表。
//! - 查不到、超时、回的不是 2xx、格式不对：当没有，记一行 `DEBUG quota unavailable`，不报错、不告警。

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use miyu_config::Values;
use miyu_config::secret::{Reference, Secret};
use miyu_http::{Get, Got, get_full};
use miyu_models::provider::{self, Provider};
use miyu_models::quota::{self, SEED, Window};

use crate::TARGET;

use super::ModelData;

/// 同一家多久内用上一次的结果。
pub const REUSE: Duration = Duration::from_secs(60);

/// 一次最多等多久。
const TIMEOUT: Duration = Duration::from_secs(10);

/// 响应体最多多少字节：几个窗口，几百字节。
const LIMIT: usize = 64 * 1024;

/// 一家上一次查的：什么时候、查到了什么（没查到的是 `None`）。
type Asked = (Instant, Option<Vec<Window>>);

/// 每一家上一次查的。
#[derive(Debug, Default)]
pub(crate) struct Quotas {
    last: Mutex<BTreeMap<String, Asked>>,
    /// 同一时刻只查一个：后到的等它查完，照它答。
    asking: tokio::sync::Mutex<()>,
}

impl Quotas {
    /// 编号 `id` 这一家 [`REUSE`] 内查过的。
    fn fresh(&self, id: &str) -> Option<Option<Vec<Window>>> {
        let last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        last.get(id)
            .filter(|(at, _)| at.elapsed() < REUSE)
            .map(|(_, windows)| windows.clone())
    }

    fn keep(&self, id: &str, windows: Option<Vec<Window>>) {
        self.last
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.to_string(), (Instant::now(), windows));
    }
}

/// 编号 `id` 这一家这时的额度，照这一刻的最终值 `values`，key、地址照 `secret` 取：档案没写 `quota` 的、这一家用不了的不查，
/// 没有；[`REUSE`] 内查过的照上一次的。
pub async fn quota(
    data: &ModelData,
    values: &Values,
    secret: &(dyn Fn(&Reference) -> Option<Secret> + Sync),
    id: &str,
) -> Option<Vec<Window>> {
    let provider = data
        .with(|knowledge| provider::provider(values, knowledge, id))
        .ok()?;
    let spec = provider.quota.clone()?;
    let quotas = data.quotas();
    let _asking = quotas.asking.lock().await;
    if let Some(windows) = quotas.fresh(id) {
        return windows;
    }
    let windows = match fetch(data, &provider, &spec, secret).await {
        Ok(windows) => Some(windows),
        Err(error) => {
            tracing::debug!(target: TARGET, provider = id, error = %error, "quota unavailable");
            None
        }
    };
    quotas.keep(id, windows.clone());
    windows
}

/// 会话一轮结束（[`crate::ModelPort::turn_ended`]）：在后台查一次，结果留着给 `session.quota`。
pub(crate) fn refresh(data: std::sync::Arc<ModelData>, config: crate::TurnConfig, id: String) {
    tokio::spawn(async move {
        let secret = |reference: &Reference| config.secret(reference);
        drop(quota(&data, &config.resolved.values(), &secret, &id).await);
    });
}

/// GET、读。
async fn fetch(
    data: &ModelData,
    provider: &Provider,
    spec: &quota::Quota,
    secret: &(dyn Fn(&Reference) -> Option<Secret> + Sync),
) -> Result<Vec<Window>, String> {
    let reference = provider.key.as_ref().ok_or("no key")?;
    let key = secret(reference).ok_or("no usable key")?;
    let mut headers = provider.headers(SEED);
    headers.push((
        "Authorization".to_string(),
        format!("Bearer {}", key.expose()),
    ));
    let client = data
        .fetcher_for(&spec.url)
        .ok_or("no client to fetch with")?;
    let got = get_full(Get {
        client,
        url: &spec.url,
        headers: &headers,
        etag: None,
        timeout: TIMEOUT,
        limit: LIMIT,
    })
    .await
    .map_err(|failed| failed.message)?;
    let Got::Body { bytes, .. } = got else {
        return Err("not modified without asking".to_string());
    };
    quota::read(spec.format, &bytes)
}
