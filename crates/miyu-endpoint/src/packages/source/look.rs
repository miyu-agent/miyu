//! `package.info` 查软件源的那一份（施工 IN-2，设计 `49-装包照pacman.md` 第一节、第三节第 2 条，照 pacman 的 `-Si`）：照从软件源
//! 装的同一套找（[`super::fetch::found`]：先拿索引，再照先后找官方的、配置里加的源），找到第一个就用它；文件挑本机能用的那一份
//! （同 [`super::fetch::pick`]）。

use std::sync::Arc;

use serde_json::{Value, json};

use super::feeds::Tier;
use crate::Core;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;

/// 软件源里编号是 `id` 的那一个：`from: "source"`、源的名字、编号、版本、名字和说明（照连接的语言挑）、依赖、推荐、带了哪几样
/// （索引原样）、要的能力（同装着的那一份的写法）、来源仓库、是不是官方；写了的有作者、上游、本机那份文件多大（`download`）、
/// 装好以后多大（`installed_size`）；用的是本机旧索引的带 `stale`（同 `package.search`）。
///
/// # Errors
///
/// 同从软件源装：哪个源都没有的 `unknown_package`，官方的拿不成、别的源也没有的照官方的拒（`source_unreachable`）。
pub(crate) async fn look(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let (feed, synced) = super::fetch::found(core, id, None).await?;
    let entry = synced.index.entry(id).ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let language = peer.language;
    let capabilities = entry.capabilities.iter().map(String::as_str);
    let mut answer = json!({
        "from": "source",
        "source": feed.name,
        "package": entry.id,
        "version": entry.version,
        "depends": entry.depends,
        "recommends": entry.recommends,
        "provides": entry.provides,
        "capabilities": crate::extensions::named(core, capabilities, language),
        "origin": entry.origin,
        // 别的源里的不算官方（施工 U23 下下）：那一格是它的索引自己写的。
        "official": entry.official && feed.tier == Tier::Official,
    });
    for (key, value) in [
        ("name", pick(&entry.name, language)),
        ("summary", pick(&entry.summary, language)),
        ("author", entry.author.clone()),
        ("upstream", entry.upstream.clone()),
    ] {
        if let Some(value) = value {
            answer[key] = json!(value);
        }
    }
    if let Some(file) = super::fetch::pick(entry) {
        answer["download"] = json!(file.size);
    }
    if let Some(bytes) = entry.installed {
        answer["installed_size"] = json!(bytes);
    }
    if let Some(stale) = synced.stale {
        answer["stale"] = Value::String(stale);
    }
    Ok(answer)
}
