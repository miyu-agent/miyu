//! 在软件源里搜（施工 G-3 下，设计 `37-软件源.md` 第四节第 3 条）：`package.search {"query"?}`。先照 [`super::sync`] 拿索引
//! （本机那份不到 24 小时的直接用），再在索引里照编号、名字、说明搜：几个词都得出现，不分大小写；空的列出全部。
//!
//! 每一条带人要的：编号、名字、说明（照连接的语言挑）、装着的版本、是不是官方、被拉黑的理由；版本也给（升级、头要比）。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_source::index::Entry;

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

/// `package.search`：回 `{"packages": [...], "generated", "stale"?}`。
pub(crate) async fn search(
    core: &Arc<Core>,
    peer: Peer,
    params: SearchParams,
) -> Result<Value, Refusal> {
    let synced = super::sync::sync(core, false).await?;
    let installed: BTreeMap<String, Option<String>> = core
        .packages()
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
    let rows: Vec<Value> = synced
        .index
        .packages
        .iter()
        .filter(|entry| matches(entry, &words))
        .map(|entry| {
            json!({
                "package": entry.id,
                "version": entry.version,
                "name": pick(&entry.name, peer.language),
                "summary": pick(&entry.summary, peer.language),
                "official": entry.official,
                "installed": installed.get(&entry.id).map(|version| version.clone().unwrap_or_default()),
                "blocked": synced.index.blocked(&entry.id, &entry.origin).map(|blocked| blocked.reason.clone()),
            })
        })
        .collect();
    let mut answer = json!({"packages": rows, "generated": synced.index.generated});
    if let Some(stale) = synced.stale {
        answer["stale"] = Value::String(stale);
    }
    Ok(answer)
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
