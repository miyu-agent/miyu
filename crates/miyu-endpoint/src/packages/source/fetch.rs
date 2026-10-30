//! 从软件源装（施工 G-4 上，设计 `37-软件源.md` 第四节第 4 条）：`package.install {"source": "<编号>", "preview"?}`。
//!
//! - 先照 [`super::sync`] 拿索引；编号不在索引里的 `unknown_package`。
//! - 依赖照 pacman 自动补：缺的依赖从索引里一起装，先装依赖；索引里也没有的 `depends_missing`（`data.missing`）。
//! - 每个包挑本机能用的那个文件（不分系统的、或者写了本机这一种的），没有的 `wrong_target`。
//! - 看一眼只照索引说要装哪几个、要下多大、装好以后多大、推荐的还没装的有哪些（施工 IN-3），不下载。
//! - 真装：一次只装一件（`core.packaging`）。照 pacman 分三段（施工 IN-3，设计 `49-装包照pacman.md` 第二节）：先一个一个下完
//!   （[`super::download`]），再一个一个核对 SHA-256，最后一个一个照装包文件那一条装（[`super::super::archive`]），本地库记上
//!   来源仓库；每段的进度推 `package.progress` 给发起的连接（[`super::super::progress`]）。下不成、核对不过的
//!   `download_failed`（`data.why` 是每一处的原因），一个都不装；装到一半出错的，前面装好的留着。

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::Target;
use miyu_source::archive;
use miyu_source::index::{Entry, File, Index};
use miyu_store::packages::Packages;

use super::super::progress::{self, At, Stage};
use super::super::{TARGET, packages};
use super::download::{self, Wanted};
use super::feeds::{Feed, Tier};
use super::sync::Synced;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::reverse::Peer as Reverse;

/// `package.install` 从软件源装的参数：`source` 写编号；只写 `package` 的、那个编号又不是卸掉的出厂包的也走这里（施工 G-4 下：
/// 装只有一个动词，核心判断是装回出厂的还是从软件源装）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceParams {
    /// 索引里的编号。
    #[serde(alias = "package")]
    source: String,
    /// 从哪个软件源装（施工 U23 下下）：配置里起的名字、`official`；不写的照先后找（官方的、系统配置加的、自己加的）。
    #[serde(default)]
    from: Option<String>,
    /// 只看一眼。
    #[serde(default)]
    preview: bool,
}

impl SourceParams {
    /// 真装编号是 `id` 的（升级照它一个一个换，施工 G-5）。
    pub(crate) fn of(id: &str) -> SourceParams {
        SourceParams {
            source: id.to_string(),
            from: None,
            preview: false,
        }
    }

    /// 从源 `feed` 真装编号是 `id` 的（施工 U23 下下：升级照装的时候那个源换）。
    pub(crate) fn from(id: &str, feed: &str) -> SourceParams {
        SourceParams {
            source: id.to_string(),
            from: Some(feed.to_string()),
            preview: false,
        }
    }

    /// 看一眼编号是 `id` 的（她的工具问人之前，施工 G-6）。
    pub(crate) fn looking(id: &str) -> SourceParams {
        SourceParams {
            source: id.to_string(),
            from: None,
            preview: true,
        }
    }
}

/// 这一条 `package.install` 是不是从软件源装：写了 `source` 的；只写了 `package`、那个编号不是出厂包的（出厂包的编号照旧是
/// 装回来：卸掉了的装回来，没卸的 `unknown_package`；软件源里也不会有同编号的，装上会撞 `package_exists`）。
pub(crate) fn wanted(core: &Core, params: &Value) -> bool {
    if params.get("source").is_some() {
        return true;
    }
    match (
        params.get("package").and_then(Value::as_str),
        params.get("path"),
    ) {
        // 组名（施工 BS-1 下中）照装回出厂的那一路走。
        (Some(id), None) => {
            !Packages::shipped(&core.resources)
                .read()
                .iter()
                .any(|shipped| shipped.id == id)
                && !crate::packages::group::is_group(core, id)
        }
        _ => false,
    }
}

/// 要装的一个：从哪个源、索引里那一条、挑好的文件、文件名。
pub(super) struct Step<'a> {
    pub(super) feed: &'a Feed,
    pub(super) entry: &'a Entry,
    pub(super) file: &'a File,
    name: String,
}

/// 从软件源装 `params.source`。
pub(crate) async fn install(
    core: &Arc<Core>,
    peer: Peer,
    caller: Option<&Reverse>,
    params: SourceParams,
) -> Result<Value, Refusal> {
    if !miyu_store::personas::valid(&params.source) {
        return Err(Refusal::BAD_PARAMS);
    }
    let indexes = chosen(core, &params).await?;
    let seen = core.system_packages();
    let installed: BTreeSet<String> = seen.iter().map(|found| found.id.clone()).collect();
    let steps = plan(&indexes, &params.source, &installed)?;
    if params.preview {
        return Ok(super::glance::preview(
            (core, peer.language),
            &indexes,
            &steps,
            &seen,
        ));
    }
    let _one_at_a_time = core.packaging.lock().await;
    let cache = download::cache_dir(packages(core).state_root());
    let count = steps.len();
    let mut files = Vec::with_capacity(count);
    for (n, step) in steps.iter().enumerate() {
        let report = progress::downloading(caller, &step.entry.id, At::nth(n, count));
        let wanted = Wanted {
            name: &step.name,
            file: step.file,
            mirrors: step.feed.mirrors(),
        };
        let file = download::fetch(&cache, &wanted, &report)
            .await
            .map_err(Refusal::download_failed)?;
        files.push(file);
    }
    for (n, (step, file)) in steps.iter().zip(&files).enumerate() {
        progress::stage(caller, &step.entry.id, Stage::Verify, At::nth(n, count));
        download::verify(file, step.file)
            .await
            .map_err(Refusal::download_failed)?;
    }
    let mut answer = Value::Null;
    let mut with = Vec::new();
    for (n, (step, file)) in steps.iter().zip(&files).enumerate() {
        progress::stage(caller, &step.entry.id, Stage::Install, At::nth(n, count));
        answer = super::super::archive::install(
            core,
            peer,
            file,
            false,
            Some(step.entry.id.as_str()),
            None,
        )
        .await?;
        let (record, id, origin) = (
            packages(core).local_root(),
            step.entry.id.clone(),
            step.entry.origin.clone(),
        );
        let feed = (step.feed.tier != Tier::Official).then(|| step.feed.name.clone());
        let recorded = tokio::task::spawn_blocking(move || {
            super::super::local::set_origin(&record, &id, (&origin, feed.as_deref()))
        })
        .await
        .map_err(|_| Refusal::INTERNAL)?;
        if let Err(error) = recorded {
            tracing::warn!(target: TARGET, package = step.entry.id.as_str(), error = %error, "package origin not recorded");
        }
        if step.entry.id != params.source {
            with.push(step.entry.id.clone());
        }
    }
    if !with.is_empty() {
        answer["with"] = json!(with);
    }
    Ok(answer)
}

/// 装要用的几个源的索引（施工 U23 下下）：挑中的那个在前，依赖在它里面找不到的再照官方的找。
pub(super) type Indexes = Vec<(Feed, Index)>;

/// 挑从哪个源装（[`found`]），依赖在它里面找不到的再照官方的找：非官方的另带上官方的索引。
async fn chosen(core: &Arc<Core>, params: &SourceParams) -> Result<Indexes, Refusal> {
    let (feed, synced) = found(core, &params.source, params.from.as_deref()).await?;
    let mut indexes = vec![(feed.clone(), synced.index)];
    if feed.tier != Tier::Official {
        let official = super::feeds::official(core);
        if let Ok(synced) = super::sync::sync_feed(&official, false).await {
            indexes.push((official, synced.index));
        }
    }
    Ok(indexes)
}

/// 编号 `id` 在哪个源里：写了 `from` 的只看那一个，没写的照先后（官方的、配置里加的）找第一个有这个编号的，交回那个源和它的
/// 索引（`package.info` 查软件源也照它找，施工 IN-2）。官方的拿不成、别的源也没有的照官方的拒；`from` 写的源没有的
/// `unknown_source`；都没有的 `unknown_package`。别的源拿不成的跳过，记一行。
pub(crate) async fn found(
    core: &Arc<Core>,
    id: &str,
    from: Option<&str>,
) -> Result<(Feed, Synced), Refusal> {
    let feeds = super::feeds::feeds(core);
    let wanted: Vec<&Feed> = feeds
        .iter()
        .filter(|feed| from.is_none_or(|name| feed.name == name))
        .collect();
    if wanted.is_empty() {
        return Err(Refusal::UNKNOWN_SOURCE);
    }
    let mut failed = None;
    for feed in wanted {
        match super::sync::sync_feed(feed, false).await {
            Ok(synced) if synced.index.entry(id).is_some() => return Ok((feed.clone(), synced)),
            Ok(_) => {}
            Err(refusal) if feed.tier == Tier::Official || from.is_some() => {
                failed.get_or_insert(refusal);
            }
            Err(refusal) => {
                tracing::info!(target: TARGET, source = feed.name.as_str(), reason = refusal.reason, "package source skipped");
            }
        }
    }
    Err(failed.unwrap_or(Refusal::UNKNOWN_PACKAGE))
}

/// 照依赖排好先后：依赖在前，要装的那个最后；装着的依赖不再装。要装的照 `indexes` 头一个找，依赖照先后找。
fn plan<'a>(
    indexes: &'a Indexes,
    id: &str,
    installed: &BTreeSet<String>,
) -> Result<Vec<Step<'a>>, Refusal> {
    let (feed, index) = indexes.first().ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let target = index.entry(id).ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let mut order: Vec<(&Feed, &Entry)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    visit(
        indexes,
        (feed, target),
        installed,
        &mut seen,
        &mut order,
        &mut missing,
    );
    if !missing.is_empty() {
        let missing: Vec<&str> = missing.iter().map(String::as_str).collect();
        return Err(Refusal::depends_missing(&missing));
    }
    order
        .into_iter()
        .map(|(feed, entry)| {
            let file = pick(entry).ok_or(Refusal::WRONG_TARGET)?;
            let target = file.target.as_deref().and_then(Target::parse);
            Ok(Step {
                feed,
                entry,
                file,
                name: archive::file_name(&entry.id, &entry.version, target),
            })
        })
        .collect()
}

/// 深度优先：先排它的依赖，再排它自己；排过的、装着的依赖跳过（依赖成环的也停得下来）。
fn visit<'a>(
    indexes: &'a Indexes,
    (feed, entry): (&'a Feed, &'a Entry),
    installed: &BTreeSet<String>,
    seen: &mut BTreeSet<String>,
    order: &mut Vec<(&'a Feed, &'a Entry)>,
    missing: &mut Vec<String>,
) {
    if !seen.insert(entry.id.clone()) {
        return;
    }
    for dep in &entry.depends {
        if installed.contains(dep) || seen.contains(dep) {
            continue;
        }
        let found = indexes
            .iter()
            .find_map(|(feed, index)| Some((feed, index.entry(dep)?)));
        match found {
            Some(found) => visit(indexes, found, installed, seen, order, missing),
            None if !missing.contains(dep) => missing.push(dep.clone()),
            None => {}
        }
    }
    order.push((feed, entry));
}

/// 本机能用的那个文件：写了本机这一种的先，不分系统的其次。
pub(super) fn pick(entry: &Entry) -> Option<&File> {
    let here = Target::here().map(Target::as_str);
    entry
        .files
        .iter()
        .find(|file| file.target.is_some() && file.target.as_deref() == here)
        .or_else(|| entry.files.iter().find(|file| file.target.is_none()))
}

#[cfg(test)]
mod tests;
