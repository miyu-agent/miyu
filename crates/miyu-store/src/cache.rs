//! 清缓存（施工 Y-11 上，项目主人 2026-10-23 定）：账号的 `home/<账号>/cache/` 下一格一格看（`<来处>/<工具名>/`、场所会话的
//! `@<场所>/<来处>/<工具名>/`），里面最新的修改时刻离现在超过保留期的整格删；场所那一层，这个账号已经没有会话在那个场所的整层
//! 删。不设总量上限。碰磁盘，调的一方放在阻塞线程里。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use miyu_kernel::id::VenueId;

use crate::scripts::venue_of;

/// 保留期：30 天没碰过的删。
pub const KEEP: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// 清缓存目录 `cache`：现在是 `now`，保留 `keep`；`live` 说一个场所这个账号还有没有会话在。交回删了的几格和删不掉的
/// （带原因）。目录不在的什么都不做。
pub fn prune(
    cache: &Path,
    now: SystemTime,
    keep: Duration,
    live: &dyn Fn(&VenueId) -> bool,
) -> (Vec<PathBuf>, Vec<(PathBuf, std::io::Error)>) {
    let mut gone = Vec::new();
    let mut failed = Vec::new();
    let mut remove = |dir: PathBuf| match std::fs::remove_dir_all(&dir) {
        Ok(()) => gone.push(dir),
        Err(error) => failed.push((dir, error)),
    };
    for top in folders(cache) {
        let name = top
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let sources = match name
            .as_deref()
            .map(|name| (name.starts_with('@'), venue_of(name)))
        {
            // 场所那一层：没了的整层删，还在的往下照时间看。
            Some((true, Some(venue))) if !live(&venue) => {
                remove(top);
                continue;
            }
            Some((true, Some(_))) => folders(&top),
            // 写不成场所的 `@` 开头的不是我们写的：不碰。
            Some((true, None)) => continue,
            _ => vec![top],
        };
        for source in sources {
            for cell in folders(&source) {
                let stale = newest(&cell)
                    .is_none_or(|at| now.duration_since(at).is_ok_and(|age| age > keep));
                if stale {
                    remove(cell);
                }
            }
        }
    }
    (gone, failed)
}

/// 目录 `dir` 下一层的文件夹（不跟链接）；读不了的没有。
fn folders(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

/// 目录 `dir` 里（往下所有层）文件最新的修改时刻；一个文件都没有的照目录自己的。不跟链接。读不了的当没有。
fn newest(dir: &Path) -> Option<SystemTime> {
    let mut latest: Option<SystemTime> = None;
    let mut found = false;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(here) = stack.pop() {
        for entry in std::fs::read_dir(&here).into_iter().flatten().flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if let Ok(at) = entry.metadata().and_then(|metadata| metadata.modified()) {
                found = true;
                latest = latest.max(Some(at));
            }
        }
    }
    match found {
        true => latest,
        false => std::fs::metadata(dir)
            .and_then(|metadata| metadata.modified())
            .ok(),
    }
}

#[cfg(test)]
mod tests;
