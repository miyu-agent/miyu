//! 拿索引（施工 G-3 上，设计 `37-软件源.md` 第四节第 1、2 条、第五节）：照 `packages.mirrors`、`packages.source` 的先后拿
//! `index.json` 和 `index.json.sig`，照编进来的官方公钥验签，比本机那份旧的不收，存进 `state/packages/.sync/`（索引、签名照
//! 收到的字节存，另记拿到的时刻 `fetched`）。
//!
//! - 不设定时器：用到软件源的时候（`package.sync`，之后的搜、装、升级）本机那份超过 24 小时才去拿，`force` 的不管时候。
//! - 一处不成试下一处；全不成的照旧用本机那份，带上最后一处的原因（`stale`）；本机也没有的拒 `source_unreachable`。
//! - 本机存的那份每次读都再验一遍签名：数据根里的文件谁都可能改。
//! - 几个源各拿各的（施工 U23 下下，[`super::feeds`]）：每个源照它自己的地址、公钥、目录，个人层的照地址闸只连公网（[`guarded`]）。
//!   `package.sync` 拿官方的，加系统配置里的、调的人个人设置里的；别的源拿不成不耽误官方那一份。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use jiff::{SignedDuration, Timestamp};
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Address;
use miyu_http::{Get, Got, Proxy};
use miyu_kernel::id::AccountId;
use miyu_source::index::{Index, may_replace};
use miyu_source::sign::{self, PublicKey};

use super::super::{TARGET, packages};
use super::feeds::Feed;
use crate::Core;
use crate::refusal::Refusal;
use crate::settings::SourceSettings;

/// 索引、签名、拿到的时刻在 `.sync` 里的文件名。
const INDEX: &str = "index.json";
const SIGNATURE: &str = "index.json.sig";
const FETCHED: &str = "fetched";

/// 多久以内的不用再拿。
const FRESH: SignedDuration = SignedDuration::from_hours(24);

/// 索引、签名最多多大。
const MOST_INDEX: usize = 32 << 20;
const MOST_SIGNATURE: usize = 64 << 10;

/// 一次拿最多多久。
const WAIT: Duration = Duration::from_secs(60);

/// 认哪几把公钥：编进来的官方公钥（[`sign::OFFICIAL`]），测试换成自己的。测试另能给个人层软件源的地址闸开口子（`testkit`）。
#[derive(Debug, Clone)]
pub(crate) struct Trust {
    pub(crate) keys: Vec<PublicKey>,
    #[cfg(feature = "testkit")]
    testing: Option<miyu_http::guard::Testing>,
}

impl Default for Trust {
    fn default() -> Trust {
        Trust {
            keys: sign::official(),
            #[cfg(feature = "testkit")]
            testing: None,
        }
    }
}

impl Core {
    /// 同一份家底，软件源的索引照 `keys` 验（施工 G-3 上）：测试用自己建的钥匙签假的软件源。
    #[must_use]
    pub fn with_source_keys(mut self, keys: Vec<PublicKey>) -> Core {
        self.trust.keys = keys;
        self
    }

    /// 同一份家底，个人层软件源的地址闸照 `testing`（施工 U23 下下）：回环当公网，本机的假软件源连得上。只在测试里有。
    #[cfg(feature = "testkit")]
    #[must_use]
    pub fn with_source_guard(mut self, testing: miyu_http::guard::Testing) -> Core {
        self.trust.testing = Some(testing);
        self
    }
}

/// 替成员连个人层软件源的那一道闸（施工 U23 下下）：平时照环境变量的代理、闸没有口子；测试照给的口子。
pub(crate) fn guarded(core: &Core) -> miyu_http::Guarded {
    #[cfg(feature = "testkit")]
    if let Some(testing) = &core.trust.testing {
        return miyu_http::Guarded::testing(testing.clone());
    }
    let _ = core;
    miyu_http::Guarded::from_env()
}

/// 拿到的索引。
#[derive(Debug, Clone)]
pub(crate) struct Synced {
    /// 索引。
    pub(crate) index: Index,
    /// 什么时候拿到的。
    pub(crate) fetched: Timestamp,
    /// 这一次没拿成、用的本机那份：最后一处的原因。
    pub(crate) stale: Option<String>,
}

/// 拿官方的索引：同 [`sync_feed`]。
///
/// # Errors
///
/// 同 [`sync_feed`]。
pub(crate) async fn sync(core: &Arc<Core>, force: bool) -> Result<Synced, Refusal> {
    sync_feed(core, &super::feeds::official(core), force).await
}

/// 拿源 `feed` 的索引：本机那份不到 24 小时的（`force` 的不算）直接用；不然照先后去拿。
///
/// # Errors
///
/// 全不成、本机也没有：`source_unreachable`（`data.why` 是最后一处的原因）。
pub(crate) async fn sync_feed(
    core: &Arc<Core>,
    feed: &Feed,
    force: bool,
) -> Result<Synced, Refusal> {
    let dir = feed.dir.clone();
    let keys = feed.keys.clone();
    let place = dir.clone();
    let kept = tokio::task::spawn_blocking(move || stored(&place, &keys))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    let now = Timestamp::now();
    if !force
        && let Some((_, fetched)) = &kept
        && now.duration_since(*fetched) < FRESH
    {
        let (index, fetched) = kept.ok_or(Refusal::INTERNAL)?;
        return Ok(Synced {
            index,
            fetched,
            stale: None,
        });
    }
    let mut last = "no source configured".to_string();
    let guarded = feed.guarded().then(|| guarded(core));
    for base in &feed.bases {
        match fetch(base, &feed.keys, guarded.as_ref()).await {
            Ok((index, bytes, signature)) => {
                if let Some((kept, _)) = &kept
                    && !may_replace(kept, &index)
                {
                    last = format!("{base}: the index is older than the one here");
                    continue;
                }
                let place = dir.clone();
                let stamp = now.to_string();
                let saved = tokio::task::spawn_blocking(move || {
                    save(
                        &place,
                        &[
                            (INDEX, &bytes),
                            (SIGNATURE, &signature),
                            (FETCHED, stamp.as_bytes()),
                        ],
                    )
                })
                .await
                .map_err(|_| Refusal::INTERNAL)?;
                if let Err(error) = saved {
                    tracing::warn!(target: TARGET, error = %error, "package index not saved");
                }
                return Ok(Synced {
                    index,
                    fetched: now,
                    stale: None,
                });
            }
            Err(why) => {
                tracing::info!(target: TARGET, source = feed.name.as_str(), why = why.as_str(), "package source not reached");
                last = format!("{base}: {why}");
            }
        }
    }
    match kept {
        Some((index, fetched)) => Ok(Synced {
            index,
            fetched,
            stale: Some(last),
        }),
        None => Err(Refusal::source_unreachable(last)),
    }
}

/// `package.sync` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncParams {
    /// 不管本机那份新不新，都去拿（`miyu pkg update`）。
    #[serde(default)]
    force: bool,
}

/// `package.sync`：拿官方的索引，回 `{"generated", "fetched", "packages", "stale"?}`；配置里还有别的源的（施工 U23 下下：系统配置
/// 里的、`account` 个人设置里的）另带 `sources: [{"source", "generated", "fetched", "packages", "stale"?}]`，拿不成、本机也没有
/// 的那一项是 `{"source", "error"}`，不耽误别的。
pub(crate) async fn method(
    core: &Arc<Core>,
    account: &AccountId,
    params: SyncParams,
) -> Result<Value, Refusal> {
    let synced = sync(core, params.force).await?;
    let mut answer = shown(&synced);
    let mut others = Vec::new();
    for feed in super::feeds::feeds(core, Some(account)).iter().skip(1) {
        let mut row = match sync_feed(core, feed, params.force).await {
            Ok(synced) => shown(&synced),
            Err(refusal) => json!({"error": refusal.reason}),
        };
        row["source"] = json!(feed.name);
        others.push(row);
    }
    if !others.is_empty() {
        answer["sources"] = Value::Array(others);
    }
    Ok(answer)
}

/// 一份拿到的索引写成回应：生成的时刻、拿到的时刻、几个包、没拿成的原因。
fn shown(synced: &Synced) -> Value {
    let mut answer = json!({
        "generated": synced.index.generated,
        "fetched": synced.fetched.to_string(),
        "packages": synced.index.packages.len(),
    });
    if let Some(stale) = &synced.stale {
        answer["stale"] = Value::String(stale.clone());
    }
    answer
}

/// 官方那一个照先后试的地址：镜像，再是软件源；写 `{ env = … }` 的照核心的环境取，取不到的跳过。都补上结尾的 `/`。
pub(super) fn bases(settings: &SourceSettings) -> Vec<String> {
    resolved(
        settings
            .mirrors
            .iter()
            .chain(std::iter::once(&settings.source)),
    )
}

/// 几个地址照先后取成网址：写 `{ env = … }` 的照核心的环境取，取不到的、空的跳过，都补上结尾的 `/`。下包文件的镜像也照它
/// 取（施工 G-4 上）。
pub(super) fn resolved<'a>(addresses: impl Iterator<Item = &'a Address>) -> Vec<String> {
    addresses
        .filter_map(|address| match address {
            Address::Literal(url) => Some(url.clone()),
            Address::Env(name) => std::env::var(name).ok(),
        })
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
        .map(|url| {
            if url.ends_with('/') {
                url
            } else {
                format!("{url}/")
            }
        })
        .collect()
}

/// 从 `base` 拿索引和签名、验过、读成：交回读好的索引和收到的两份字节。`guarded` 的照地址闸只连公网（个人层的源）。
async fn fetch(
    base: &str,
    keys: &[PublicKey],
    guarded: Option<&miyu_http::Guarded>,
) -> Result<(Index, Vec<u8>, Vec<u8>), String> {
    if let Some(guarded) = guarded {
        let bytes = guarded
            .get(&format!("{base}{INDEX}"), MOST_INDEX, WAIT)
            .await?;
        let signature = guarded
            .get(&format!("{base}{SIGNATURE}"), MOST_SIGNATURE, WAIT)
            .await?;
        sign::verify(&bytes, &signature, keys).map_err(|error| error.to_string())?;
        let index = Index::parse(&bytes).map_err(|error| error.to_string())?;
        return Ok((index, bytes, signature));
    }
    let proxy = if miyu_http::is_loopback_url(base) {
        Proxy::Off
    } else {
        Proxy::FromEnvironment
    };
    let client = miyu_http::fetcher(proxy).map_err(|error| error.to_string())?;
    let bytes = get(&client, &format!("{base}{INDEX}"), MOST_INDEX).await?;
    let signature = get(&client, &format!("{base}{SIGNATURE}"), MOST_SIGNATURE).await?;
    sign::verify(&bytes, &signature, keys).map_err(|error| error.to_string())?;
    let index = Index::parse(&bytes).map_err(|error| error.to_string())?;
    Ok((index, bytes, signature))
}

async fn get(client: &reqwest::Client, url: &str, limit: usize) -> Result<Vec<u8>, String> {
    let got = miyu_http::get(Get {
        client,
        url,
        headers: &[],
        etag: None,
        timeout: WAIT,
        limit,
    })
    .await?;
    match got {
        Got::Body { bytes, .. } => Ok(bytes),
        Got::NotModified => Err("not modified".to_string()),
    }
}

/// 本机存的那份索引里的包编号（施工 Y-5 中下：注册时这些名字算占着）：不联网，没拿过、验不过的是空的。碰磁盘。
pub(crate) fn indexed_ids(core: &Core) -> Vec<String> {
    let dir = packages(core).state_root().join(".sync");
    stored(&dir, &core.trust.keys).map_or_else(Vec::new, |(index, _)| {
        index.packages.into_iter().map(|entry| entry.id).collect()
    })
}

/// 本机存的那份：签名验得过、读得成的才算；拿到的时刻读不成的当很久以前。
fn stored(dir: &Path, keys: &[PublicKey]) -> Option<(Index, Timestamp)> {
    let bytes = std::fs::read(dir.join(INDEX)).ok()?;
    let signature = std::fs::read(dir.join(SIGNATURE)).ok()?;
    sign::verify(&bytes, &signature, keys).ok()?;
    let index = Index::parse(&bytes).ok()?;
    let fetched = std::fs::read_to_string(dir.join(FETCHED))
        .ok()
        .and_then(|text| text.trim().parse::<Timestamp>().ok())
        .unwrap_or(Timestamp::UNIX_EPOCH);
    Some((index, fetched))
}

/// 几份一起写：先写旁边点开头的临时文件，都写成了再一个个换过去。
fn save(dir: &Path, files: &[(&str, &[u8])]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (name, bytes) in files {
        let temp = dir.join(format!(".{name}.new"));
        std::fs::write(&temp, bytes)?;
        staged.push((temp, dir.join(name)));
    }
    for (temp, target) in staged {
        std::fs::rename(temp, target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
