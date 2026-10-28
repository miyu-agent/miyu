//! 脚本跑完清一次这个人的缓存（施工 Y-11 上，`miyu_store::cache`）：照时间（30 天没碰的整格删），场所没了的那一层留给核心起来时清
//! （它查得到会话索引）。同一个缓存目录一分钟内只清一次，不为每次调用都扫一遍；在阻塞线程里清，不等它。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use miyu_tool::Call;

/// 同一个缓存目录多久清一次。
const EVERY: Duration = Duration::from_secs(60);

/// 每个缓存目录上一次清的时刻。
static LAST: Mutex<BTreeMap<PathBuf, Instant>> = Mutex::new(BTreeMap::new());

/// 调用 `call` 跑完了：缓存目录离上一次清满了 [`EVERY`] 的，在阻塞线程里清一次。没有属主、数据根的什么都不做。
pub(super) fn after(call: &Call) {
    let (Some(root), true) = (call.data_root.as_deref(), call.ids.is_some()) else {
        return;
    };
    let cache = root.join(miyu_store::root::CACHE);
    {
        let mut last = LAST.lock().unwrap_or_else(PoisonError::into_inner);
        if last.get(&cache).is_some_and(|at| at.elapsed() < EVERY) {
            return;
        }
        last.insert(cache.clone(), Instant::now());
    }
    drop(tokio::task::spawn_blocking(move || {
        let (gone, failed) =
            miyu_store::cache::prune(&cache, SystemTime::now(), miyu_store::cache::KEEP, &|_| {
                true
            });
        if !gone.is_empty() {
            tracing::info!(target: "miyu::shell", removed = gone.len(), "script cache pruned");
        }
        if !failed.is_empty() {
            tracing::warn!(target: "miyu::shell", failed = failed.len(), "script cache entries kept");
        }
    }));
}
