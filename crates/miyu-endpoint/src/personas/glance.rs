//! 人格图片一眼看得出的两样：版本（`persona.list`、`persona.get` 的 `avatar`、`background`，图的字节的哈希取前 16 位）和
//! 头像的候选主色（`seeds`，施工 P-6 补，`docs/blueprint/personas.md`「主题色、背景图」第 4 条：照 `miyu-seeds` 挑最多 4 个）。
//!
//! 算一次要整份读图、解图：debug 编的核心解一张 577 KB 的 JPEG 要 0.77 秒，release 编 0.04 秒（施工 V-16 量的）。原来只记在
//! 这个进程里，核心一重启第一次 `persona.list` 就要现场算，终端的人格框跟着等。现在记在 `state/personas/pictures.json`：照图的
//! 位置、大小、修改时刻认，三样都对得上的照记下的给，不读图；对不上的（换了图）重算、写回。它是派生的数据，坏了、删了照空的
//! 算，下一次重算。读盘、解图，在阻塞线程里调。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use miyu_kernel::id::ContentHash;

/// 记在 `state/` 下面的哪个文件。
const FILE: &str = "personas/pictures.json";

/// 最多记几张图：人格就那么几个，超了整个清掉重来。
const KEEP: usize = 256;

/// 一张图看出来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Glance {
    /// 版本：图的字节的哈希取前 16 位。
    pub(crate) version: String,
    /// 候选主色，小写的 `#rrggbb`；没要的、解不开的是 `None`，全透明的是空的。
    pub(crate) seeds: Option<Vec<String>>,
}

/// 记下的一张。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    /// 字节数。
    size: u64,
    /// 修改时刻，纪元以来的纳秒，写成字。
    modified: String,
    /// 版本。
    version: String,
    /// 挑过候选主色了。
    #[serde(default)]
    seeded: bool,
    /// 候选主色：挑过、解得开的才有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seeds: Option<Vec<String>>,
}

/// 记下的全部：照图的位置。
#[derive(Debug, Default, Serialize, Deserialize)]
struct Book {
    pictures: BTreeMap<String, Entry>,
}

/// 这个进程里的一份：第一次用时从 `state/` 读。
static BOOK: LazyLock<Mutex<Option<(PathBuf, Book)>>> = LazyLock::new(|| Mutex::new(None));

/// 看一张图：`state` 是数据根的 `state/`，`path` 是图，`seeds` 要不要挑候选主色。图不在、读不了的是 `None`。
pub(crate) fn look(state: &Path, path: &Path, seeds: bool) -> Option<Glance> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |at| at.as_nanos())
        .to_string();
    let key = path.to_string_lossy().into_owned();
    if let Some(entry) = remembered(state, &key)
        && entry.size == meta.len()
        && entry.modified == modified
        && (!seeds || entry.seeded)
    {
        return Some(Glance {
            version: entry.version,
            seeds: entry.seeds.filter(|_| seeds),
        });
    }
    let bytes = std::fs::read(path).ok()?;
    let version = ContentHash::of(&bytes).hex()[..16].to_string();
    let picked = seeds.then(|| {
        miyu_seeds::from_image(&bytes).map(|found| {
            found
                .into_iter()
                .map(miyu_seeds::Seed::hex)
                .collect::<Vec<_>>()
        })
    });
    remember(
        state,
        key,
        Entry {
            size: meta.len(),
            modified,
            version: version.clone(),
            seeded: picked.is_some(),
            seeds: picked.clone().flatten(),
        },
    );
    Some(Glance {
        version,
        seeds: picked.flatten(),
    })
}

/// 记下的那一张：没有的是空的。
fn remembered(state: &Path, key: &str) -> Option<Entry> {
    let mut book = book();
    loaded(&mut book, state).pictures.get(key).cloned()
}

/// 记下一张，写回 `state/`。写不下的不要紧：下一次重算。
fn remember(state: &Path, key: String, entry: Entry) {
    let mut book = book();
    let loaded = loaded(&mut book, state);
    if loaded.pictures.len() >= KEEP && !loaded.pictures.contains_key(&key) {
        loaded.pictures.clear();
    }
    loaded.pictures.insert(key, entry);
    if let Err(error) = save(state, loaded) {
        tracing::debug!(target: "miyu::endpoint", error = %error, "persona pictures not remembered");
    }
}

/// 这个进程里的一份，照 `state` 读好；换了数据根（测试里）的重读。
fn loaded<'a>(book: &'a mut Option<(PathBuf, Book)>, state: &Path) -> &'a mut Book {
    if book.as_ref().is_some_and(|(at, _)| at != state) {
        *book = None;
    }
    let (_, read) = book.get_or_insert_with(|| {
        let read = std::fs::read(state.join(FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        (state.to_path_buf(), read)
    });
    read
}

/// 整份写回：先写临时文件再换名，读的一方看不到写了一半的。
fn save(state: &Path, book: &Book) -> std::io::Result<()> {
    let path = state.join(FILE);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec(book)?)?;
    std::fs::rename(temp, path)
}

/// 这一份的锁；别的线程拿着锁时崩了也照用（里面只是缓存）。
fn book() -> std::sync::MutexGuard<'static, Option<(PathBuf, Book)>> {
    BOOK.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
