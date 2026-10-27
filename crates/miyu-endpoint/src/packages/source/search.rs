//! 在软件源里搜（施工 G-3 下，设计 `37-软件源.md` 第四节第 3 条）：`package.search {"query"?}`。先照 [`super::sync`] 拿索引
//! （本机那份不到 24 小时的直接用），再在索引里照编号、名字、说明搜：几个词都得出现，不分大小写；空的列出全部。
//!
//! 每一条带人要的：编号、名字、说明（照连接的语言挑）、装着的版本、是不是官方、被拉黑的理由；版本也给（升级、头要比）；给哪几个
//! 包带了内容（`contents`，施工 G-9：im 的商城只列 im 的插件）；带了哪几样、依赖哪几个包（`provides`、`depends`，施工 G-10：
//! 软件市场照种类筛）。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_source::index::{Entry, Index};

use super::feeds::OFFICIAL;
use crate::Core;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;

/// `package.search` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchParams {
    /// 搜什么：几个词，空白分开；不写、空的列出全部。
    #[serde(default)]
    query: String,
}

impl SearchParams {
    /// 搜 `query`（她的工具经端口搜，施工 G-6）。
    pub(crate) fn of(query: String) -> SearchParams {
        SearchParams { query }
    }
}

/// `package.search`：回 `{"packages": [...], "generated", "stale"?}`。装没装照 `account` 看得到的（施工 U23 下上）。照先后搜官方的、
/// 系统配置加的、他自己加的源（施工 U23 下下），每一行带 `source`（源的名字）；别的源里的 `official` 一律是假的，同编号的各列各的。
/// `generated`、`stale` 照官方的；别的源拿不成的跳过，记一行。
pub(crate) async fn search(
    core: &Arc<Core>,
    (peer, account): (Peer, &AccountId),
    params: SearchParams,
) -> Result<Value, Refusal> {
    let synced = super::sync::sync(core, false).await?;
    let installed: BTreeMap<String, Option<String>> = core
        .packages_seen(account)
        .iter()
        .map(|found| {
            let version = found.read.as_ref().ok().and_then(|m| m.version.clone());
            (found.id.clone(), version)
        })
        .collect();
    let words: Vec<String> = params
        .query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    let mut rows = matching((OFFICIAL, &synced.index), &installed, &words, peer.language);
    for feed in super::feeds::feeds(core, Some(account)).iter().skip(1) {
        match super::sync::sync_feed(core, feed, false).await {
            Ok(other) => rows.extend(matching(
                (&feed.name, &other.index),
                &installed,
                &words,
                peer.language,
            )),
            Err(refusal) => {
                tracing::info!(target: super::super::TARGET, source = feed.name.as_str(), reason = refusal.reason, "package source skipped");
            }
        }
    }
    let mut answer = json!({"packages": rows, "generated": synced.index.generated});
    if let Some(stale) = synced.stale {
        answer["stale"] = Value::String(stale);
    }
    Ok(answer)
}

/// 源 `source`（名字）的索引 `index` 里合得上 `words` 的那几行。
fn matching(
    (source, index): (&str, &Index),
    installed: &BTreeMap<String, Option<String>>,
    words: &[String],
    language: &str,
) -> Vec<Value> {
    index
        .packages
        .iter()
        .filter(|entry| matches(entry, words))
        .map(|entry| {
            let mut row = json!({
                "package": entry.id,
                "version": entry.version,
                "name": pick(&entry.name, language),
                "summary": pick(&entry.summary, language),
                "official": entry.official && source == OFFICIAL,
                "source": source,
                // 给哪几个包带了内容（施工 G-9）：索引的 `provides` 里的 `content`；没带的是空的。
                "contents": entry.provides.get("content").cloned().unwrap_or_default(),
                // 带了哪几样、依赖哪几个包（施工 G-10，网页的软件市场照种类筛）：索引里的原样，没有的是空的。
                "provides": entry.provides,
                "depends": entry.depends,
                "installed": installed.get(&entry.id).map(|version| version.clone().unwrap_or_default()),
                "blocked": index.blocked(&entry.id, &entry.origin).map(|blocked| blocked.reason.clone()),
            });
            // 包的种类（施工 G-11）：索引里的原样，以前生成的索引没有的不写。
            if let Some(kind) = &entry.kind {
                row["kind"] = json!(kind);
            }
            row
        })
        .collect()
}

/// 每个词都出现在编号、名字、说明（哪种语言都算）里。
fn matches(entry: &Entry, words: &[String]) -> bool {
    let haystack = std::iter::once(entry.id.as_str())
        .chain(entry.name.values().map(String::as_str))
        .chain(entry.summary.values().map(String::as_str))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    words.iter().all(|word| haystack.contains(word.as_str()))
}
