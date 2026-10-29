//! 软件源里的包自带的图标（施工 F-18 下）：`package.logo {"package", "from"}` 写了 `from` 的走这里。照那个源的索引找这个包的
//! `logo`（位置、哈希、大小），照源的地址先后下 `<地址><位置>`（官方的先镜像），大小、哈希对上才收；下好的放那个源自己的目录
//! `logos/` 下，下一次哈希对得上的不再下。
//!
//! - 别人的软件源写的索引不全信：位置不是 `logos/<哈希>.png`、`.webp` 的，索引写的大小超过 [`LOGO_MAX_BYTES`] 的当没有图标，不下。
//! - 下来的再照装着的包那一套认（PNG / WebP、边长）：不合的当没有图标。
//! - 存下一张以后，目录里索引已经不提的旧图标删掉。

use std::path::Path;
use std::sync::Arc;

use miyu_config::package::{LOGO_MAX_BYTES, LOGO_MAX_SIDE};
use miyu_source::index::{File, Index, Logo};

use super::download::{self, Wanted};
use crate::Core;
use crate::refusal::Refusal;

/// 源 `from` 里编号 `id` 的包自带的图标：字节和媒体类型；索引里这个包没有图标、图标不合规矩的没有。
///
/// # Errors
///
/// 没有这个源 `unknown_source`；源拿不成照 [`super::sync::sync_feed`]；索引里没有这个编号 `unknown_package`；哪一处都没下成
/// `download_failed`。
pub(crate) async fn of(
    core: &Arc<Core>,
    (id, from): (&str, &str),
) -> Result<Option<(Vec<u8>, &'static str, String)>, Refusal> {
    let feed = super::feeds::feeds(core)
        .into_iter()
        .find(|feed| feed.name == from)
        .ok_or(Refusal::UNKNOWN_SOURCE)?;
    let synced = super::sync::sync_feed(&feed, false).await?;
    let entry = synced.index.entry(id).ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let Some(logo) = entry
        .logo
        .clone()
        .filter(|logo| logo.well_formed() && logo.size <= LOGO_MAX_BYTES)
    else {
        return Ok(None);
    };
    let cache = feed.dir.join("logos");
    let name = logo.path.trim_start_matches("logos/").to_string();
    let mut reasons = Vec::new();
    let mut got = None;
    for base in &feed.bases {
        let file = File {
            target: None,
            url: format!("{base}{}", logo.path),
            sha256: logo.sha256.clone(),
            size: logo.size,
        };
        let wanted = Wanted {
            name: &name,
            file: &file,
            mirrors: &[],
        };
        match download::fetch(&cache, &wanted, &|_, _| {}).await {
            Ok(path) => {
                got = Some(path);
                break;
            }
            Err(why) => reasons.push(why),
        }
    }
    let path = got.ok_or_else(|| Refusal::download_failed(reasons.join("; ")))?;
    let index = synced.index;
    tokio::task::spawn_blocking(move || {
        prune(&cache, &index);
        checked(&path, &logo)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)
}

/// 下好的 `path` 照装着的包那一套认：PNG / WebP、边长不超；交回字节、媒体类型、版本。
fn checked(path: &Path, logo: &Logo) -> Option<(Vec<u8>, &'static str, String)> {
    let bytes = std::fs::read(path).ok()?;
    let media_type = miyu_tool::picture::kind(&bytes)
        .filter(|kind| matches!(*kind, "image/png" | "image/webp"))?;
    let (width, height) = miyu_tool::picture::measure(&bytes)?;
    (width <= LOGO_MAX_SIDE && height <= LOGO_MAX_SIDE)
        .then(|| (bytes, media_type, logo.version().to_string()))
}

/// `cache` 里索引 `index` 已经不提的图标删掉；删不掉的留着。
fn prune(cache: &Path, index: &Index) {
    let Ok(entries) = std::fs::read_dir(cache) else {
        return;
    };
    let kept: Vec<String> = index
        .packages
        .iter()
        .filter_map(|entry| entry.logo.as_ref())
        .map(|logo| logo.path.trim_start_matches("logos/").to_string())
        .collect();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !kept.contains(&name)
            && let Err(error) = std::fs::remove_file(entry.path())
        {
            tracing::debug!(target: super::super::TARGET, error = %error, "old package logo left");
        }
    }
}
