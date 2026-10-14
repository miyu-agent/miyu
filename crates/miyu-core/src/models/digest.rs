//! 整理好的目录放在缓存目录里（施工 V-5 中，`docs/blueprint/models.md`「怎么走」第二条第 1 条）：原文现解约 22 ms，核心
//! 每次起来都要解，又正赶上头连上来载入会话、和读日志抢 CPU。解过一次就把整理好的一份（`miyu_models::catalog::Catalog::
//! digest`）写进缓存目录，下次照它读，约 7 ms。
//!
//! 认不认这一份照指纹：原文是哪一份、多大、什么时候改的、`fetched`，程序多大、什么时候改的（程序换了，读原文的写法可能
//! 变了，重解一次）。不知道程序在哪、没有缓存目录的不存不读。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use miyu_models::catalog::{Catalog, CatalogSource};

use crate::TARGET;

/// 缓存目录里的文件名。
pub const FILE: &str = "models-dev.digest.json";

/// 这一份原文的指纹：`source` 是哪一份，`file` 是原文，`fetched` 是它的 `meta` 里写的，`program` 是程序。读不到原文、
/// 程序的大小和改动时间的没有。
pub fn fingerprint(
    source: CatalogSource,
    file: &Path,
    fetched: &str,
    program: Option<&Path>,
) -> Option<String> {
    let (len, modified) = stamp(file)?;
    let (program_len, program_modified) = stamp(program?)?;
    Some(format!(
        "{} {len} {modified} {fetched} {program_len} {program_modified}",
        source.as_str()
    ))
}

/// 一个文件的大小和改动时间（纳秒）。
fn stamp(path: &Path) -> Option<(u64, u128)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((meta.len(), modified.as_nanos()))
}

/// 缓存目录 `cache` 里指纹是 `of` 的那一份；没有的、对不上的、读不懂的没有。在阻塞线程里调。
pub fn recall(cache: &Path, of: &str) -> Option<Catalog> {
    let text = std::fs::read_to_string(cache.join(FILE)).ok()?;
    Catalog::from_digest(&text, of)
}

/// 另起一个线程把 `catalog` 整理好写进缓存目录 `cache`，指纹是 `of`：先写到旁边、再换过去。写不成的记一行 `DEBUG`，下次
/// 照原文解。交回那个线程（起不来的没有），测试等它写完。
pub fn keep(cache: PathBuf, of: String, catalog: Catalog) -> Option<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("catalog-digest".to_string())
        .spawn(move || {
            let text = catalog.digest(&of);
            let temp = cache.join(format!("{FILE}.{}.tmp", std::process::id()));
            let written = std::fs::create_dir_all(&cache)
                .and_then(|()| std::fs::write(&temp, text))
                .and_then(|()| std::fs::rename(&temp, cache.join(FILE)));
            if let Err(error) = written {
                tracing::debug!(target: TARGET, error = %error, "catalog digest not written");
            }
        })
        .ok()
}
