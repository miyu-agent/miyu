//! 读目录（`docs/blueprint/models.md`「怎么走」第二条第 1、2 条，施工 8-7）：安装包带的快照、缓存目录里后台拉的，两份比
//! `meta` 的 `fetched`，用新的；新的读不了，用另一份；都读不了，目录是空的，照样起来。
//!
//! 两份都是原样的 `api.json`，旁边一份 `<名字>.meta.json`：`{"source":<网址>,"fetched":<时刻>}`，缓存的另带 `etag`。
//! `meta` 没有、读不了的当作最旧。挑中的那一份解过一次的，下次照缓存目录里整理好的一份读（施工 V-5 中，[`super::digest`]）。

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use miyu_models::catalog::{Catalog, CatalogSource, Loaded};

use super::digest;
use crate::TARGET;

/// 目录的文件名。
pub const FILE: &str = "models-dev.json";

/// 旁边那一份的文件名。
pub const META: &str = "models-dev.meta.json";

/// 一份目录旁边的 `meta`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// 从哪拉的。
    pub source: String,
    /// 什么时候拉的：RFC 3339 的 UTC 时刻，照字的先后就是时间的先后。
    pub fetched: String,
    /// 上次回的 `ETag`：缓存的才有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}

impl Meta {
    /// 读 `dir` 里的 `meta`；没有的、坏的是空的。
    pub fn read(dir: &Path) -> Option<Meta> {
        let text = std::fs::read_to_string(dir.join(META)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// 写成 JSON。
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// 两份目录在哪：快照的目录（资源目录的 `models/`），缓存的目录（`<缓存目录>/models`，算不出来的没有）；程序在哪（整理
/// 好的一份认它，施工 V-5 中；不知道的没有，不存不读整理好的）。
#[derive(Debug, Clone)]
pub struct Places {
    /// 快照所在的目录。
    pub snapshot: PathBuf,
    /// 缓存所在的目录。
    pub cache: Option<PathBuf>,
    /// 程序的真实位置。
    pub program: Option<PathBuf>,
}

/// 测试里让读目录先睡一会儿再读（施工 TMP-3）：像门禁满载时那样慢，看核心放锁之前等不等还在读的那一次。
#[cfg(test)]
pub(crate) static SLOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 读目录：挑新的、坏的退回另一份，读完记 `INFO catalog loaded`，坏了的记 `WARN catalog unreadable`，都读不了再记
/// `WARN catalog empty`。一个模型坏了的记一行 `DEBUG`。挑中的那一份有整理好的就读它，`catalog loaded` 多一格
/// `digest=true`；没有的解完另起线程整理一份（施工 V-5 中），核心放锁之前等它写完（施工 TMP-1 补2）。在阻塞线程里调。
pub fn load(places: &Places) -> Option<Loaded> {
    read(places, true)
}

/// 同 [`load`]，目录不常驻时临时读一份（施工 V-25 下：没记过的输入、要整份目录的地方）：读完那一行记成 `DEBUG`，不然每次
/// `model.list` 都记一行 `INFO`。
pub fn load_again(places: &Places) -> Option<Loaded> {
    read(places, false)
}

/// 两份目录照新旧排好：`fetched` 大的在前，一样新的快照在前；连同各自所在的目录和 `meta`。起来时照排第一的那一份认记下的
/// 那份还能不能用（施工 V-25 下，`super::recall`）。
pub fn ranked(places: &Places) -> Vec<(CatalogSource, PathBuf, Option<Meta>)> {
    let mut candidates = vec![(CatalogSource::Snapshot, places.snapshot.clone())];
    if let Some(cache) = &places.cache {
        candidates.push((CatalogSource::Cache, cache.clone()));
    }
    let mut candidates: Vec<(CatalogSource, PathBuf, Option<Meta>)> = candidates
        .into_iter()
        .map(|(source, dir)| {
            let meta = Meta::read(&dir);
            (source, dir, meta)
        })
        .collect();
    // 新的在前：`fetched` 大的；一样新的快照在前（排序是稳定的）。
    candidates.sort_by(|a, b| fetched(&b.2).cmp(fetched(&a.2)));
    candidates
}

/// `source` 那一份所在的目录；缓存目录算不出来的没有。
pub fn dir_of(places: &Places, source: CatalogSource) -> Option<&Path> {
    match source {
        CatalogSource::Snapshot => Some(&places.snapshot),
        CatalogSource::Cache => places.cache.as_deref(),
    }
}

/// [`load`]、[`load_again`]：`startup` 为真时读完记 `INFO`，不然记 `DEBUG`。
fn read(places: &Places, startup: bool) -> Option<Loaded> {
    #[cfg(test)]
    if SLOW.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    for (source, dir, meta) in ranked(places) {
        let started = Instant::now();
        let fetched = meta.map(|meta| meta.fetched).unwrap_or_default();
        let of = places.cache.as_ref().and_then(|_| {
            digest::fingerprint(source, &dir.join(FILE), &fetched, places.program.as_deref())
        });
        if let (Some(cache), Some(of)) = (&places.cache, &of)
            && let Some(catalog) = digest::recall(cache, of)
        {
            loaded(&catalog, (source, &fetched), started, true, startup);
            return Some(Loaded {
                catalog,
                source,
                fetched,
            });
        }
        let read = std::fs::read_to_string(dir.join(FILE))
            .map_err(|error| error.to_string())
            .and_then(|text| Catalog::parse(&text));
        match read {
            Ok(read) => {
                for skipped in &read.skipped {
                    tracing::debug!(target: TARGET, entry = %skipped, "catalog entry skipped");
                }
                let catalog = read.catalog;
                loaded(&catalog, (source, &fetched), started, false, startup);
                // 写盘的线程登记下来，核心放锁之前等它写完（施工 TMP-1 补2，`crate::pending`）。
                if let (Some(cache), Some(of)) = (&places.cache, of)
                    && let Some(writing) = digest::keep(
                        cache.clone(),
                        of,
                        catalog.clone(),
                        crate::pending::working(),
                    )
                {
                    crate::pending::hold(writing);
                }
                return Some(Loaded {
                    catalog,
                    source,
                    fetched,
                });
            }
            Err(error) => {
                tracing::warn!(target: TARGET, source = source.as_str(), error = %error, "catalog unreadable");
            }
        }
    }
    tracing::warn!(target: TARGET, "catalog empty");
    None
}

/// 记一行 `catalog loaded`（`startup` 为真时 `INFO`，临时读的 `DEBUG`）；照整理好的一份读的多一格 `digest=true`。
fn loaded(
    catalog: &Catalog,
    (source, fetched): (CatalogSource, &str),
    started: Instant,
    digest: bool,
    startup: bool,
) {
    let (providers, models, ms) = (
        catalog.providers().count(),
        catalog.model_count(),
        started.elapsed().as_millis(),
    );
    let source = source.as_str();
    match (startup, digest) {
        (true, true) => {
            tracing::info!(target: TARGET, source, fetched, providers, models, ms, digest, "catalog loaded");
        }
        (true, false) => {
            tracing::info!(target: TARGET, source, fetched, providers, models, ms, "catalog loaded");
        }
        (false, _) => {
            tracing::debug!(target: TARGET, source, fetched, providers, models, ms, digest, "catalog loaded");
        }
    }
}

/// 比新旧用的：没有 `meta` 的是空字，最旧。
fn fetched(meta: &Option<Meta>) -> &str {
    meta.as_ref().map_or("", |meta| meta.fetched.as_str())
}
