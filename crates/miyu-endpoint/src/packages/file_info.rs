//! `package.info` 带 `path` 的（施工 IN-4，照 pacman 的 `-Qip`；项目主人 2026-10-31 定）：看本机一个包文件夹、包文件 `.miyu`，
//! 什么都不装。查照真装之前的那一套（[`super::preview::looked`]：写错的、撞了名的、缺依赖的照样拒），交回的照 `package.info`
//! 装着的那一份的样子：`from` 是 `file`，带了什么收在 `contains`，另带写的那个路径。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use super::archive::{self, Then};
use crate::Core;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;

/// 看 `path`（写的原样，要是绝对的）：包文件解进暂存处看、看完删掉（[`archive::install`]）。和装卸排队，拿同一把锁。
pub(super) async fn file(core: &Arc<Core>, peer: Peer, path: &str) -> Result<Value, Refusal> {
    super::may_manage::may_manage(peer.for_owner)?;
    let _one_at_a_time = core.packaging.lock().await;
    let at = Path::new(path);
    let mut answer = match archive::is_package_file(at) {
        true => archive::install(core, peer, at, (Then::Describe, None), None).await?,
        false => described(core, peer, at).await?,
    };
    answer["path"] = json!(path);
    Ok(answer)
}

/// 包文件夹 `folder` 的那一份：看一眼的那几格里编号、版本、几个文件多大、要的能力、读不成的、换下哪一份照搬，带了什么
/// （程序、子命令、技能、脚本……）收进 `contains`；另照清单补名字、说明（照连接的语言）、作者、上游、依赖、推荐，照包里的脚本、
/// 功能补模型会多的工具（`tools`，施工 PV-1）。没有的不写。
pub(super) async fn described(
    core: &Arc<Core>,
    peer: Peer,
    folder: &Path,
) -> Result<Value, Refusal> {
    let (mut carried, manifest) = super::preview::looked(core, peer, folder).await?;
    let mut answer = Map::new();
    answer.insert("from".into(), json!("file"));
    for key in [
        "package",
        "version",
        "files",
        "size",
        "capabilities",
        "skipped",
        "replaces",
    ] {
        if let Some(value) = carried.remove(key) {
            answer.insert(key.into(), value);
        }
    }
    for (key, value) in [
        ("name", pick(&manifest.name, peer.language)),
        ("summary", pick(&manifest.summary, peer.language)),
        ("author", manifest.author.clone()),
        ("upstream", manifest.upstream.clone()),
    ] {
        if let Some(value) = value {
            answer.insert(key.into(), json!(value));
        }
    }
    for (key, ids) in [
        ("depends", &manifest.depends),
        ("recommends", &manifest.recommends),
    ] {
        if !ids.is_empty() {
            answer.insert(key.into(), json!(ids));
        }
    }
    if !carried.is_empty() {
        answer.insert("contains".into(), Value::Object(carried));
    }
    // 装上以后模型会多的工具（施工 PV-1）：照包里的脚本开头、清单的功能读，同装着的那一份。
    let tools = miyu_source::tools::of(&manifest, folder);
    if !tools.is_empty() {
        answer.insert("tools".into(), json!(tools));
    }
    Ok(Value::Object(answer))
}
