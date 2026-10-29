//! 核心起来时清一次缓存（施工 Y-11 上，`miyu_store::cache`）：30 天没碰的整格删；场所那一层，会话索引里已经
//! 没有会话在那个场所的整层删。索引不在、读不出来的只照时间清（不知道哪些场所还在，宁可不删）。写了 `ready` 以后在阻塞线程里
//! 清，不耽误头连上来；核心走之前等它清完（同回收处，`trash.rs`）。运行日志只记个数，不记路径。

use std::collections::BTreeSet;
use std::time::SystemTime;

use tokio::task::JoinHandle;

use miyu_kernel::id::VenueId;
use miyu_store::index::{FILE, SessionIndex};
use miyu_store::root::DataRoot;

use crate::TARGET;

/// 在阻塞线程里清一次缓存，照现在的钟。交回等它清完的那一头。
pub(crate) fn sweep(root: DataRoot) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let (removed, failed) = sweep_now(&root, SystemTime::now());
        if removed > 0 {
            tracing::info!(target: TARGET, removed, "cache pruned");
        }
        if failed > 0 {
            tracing::warn!(target: TARGET, failed, "cache entries kept");
        }
    })
}

/// 清一次数据根 `root` 的缓存，现在是 `now`：交回删了几格、删不掉几格。
pub(crate) fn sweep_now(root: &DataRoot, now: SystemTime) -> (usize, usize) {
    let cache = root.cache();
    if !cache.is_dir() {
        return (0, 0);
    }
    let venues = live_venues(root);
    let live = |venue: &VenueId| venues.as_ref().is_none_or(|venues| venues.contains(venue));
    let (gone, kept) = miyu_store::cache::prune(&cache, now, miyu_store::cache::KEEP, &live);
    (gone.len(), kept.len())
}

/// 会话索引里还有会话在的场所；索引不在、读不出来的没有（不知道）。
fn live_venues(root: &DataRoot) -> Option<BTreeSet<VenueId>> {
    let file = root.index().join(FILE);
    if !file.is_file() {
        return None;
    }
    let (index, _) = SessionIndex::open(&file);
    let rows = index.rows().ok()?;
    Some(
        rows.values()
            .filter_map(|row| VenueId::parse(&row.venue).ok())
            .collect(),
    )
}

#[cfg(test)]
mod tests;
