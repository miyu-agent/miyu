//! 清缓存（施工 G-4 下，设计 `37-软件源.md` 第四节第 5 条，照 pacman 的 `-Sc`）：`package.clean`。`state/packages/.cache/` 里
//! 不是装着那一版的包文件删掉，下了一半没换过去的也删；回 `{"removed", "freed"}`（几个文件、几个字节）。一次只做一件（和装卸
//! 同一把锁，不删正在装的）。

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Value, json};

use super::super::packages;
use super::download;
use crate::Core;
use crate::refusal::Refusal;

/// `package.clean`。
pub(crate) async fn clean(core: &Arc<Core>) -> Result<Value, Refusal> {
    let _one_at_a_time = core.packaging.lock().await;
    let keep: BTreeSet<String> = core
        .packages()
        .iter()
        .filter_map(|found| {
            let version = found.read.as_ref().ok()?.version.clone()?;
            Some(format!("{}-{version}", found.id))
        })
        .collect();
    let cache = download::cache_dir(packages(core).state_root());
    let (removed, freed) = tokio::task::spawn_blocking(move || sweep(&cache, &keep))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    Ok(json!({"removed": removed, "freed": freed}))
}

/// 删掉缓存里不在 `keep`（`<编号>-<版本>`）里的包文件和下了一半的：交回删了几个、几个字节。读不了、删不掉的跳过。
fn sweep(cache: &std::path::Path, keep: &BTreeSet<String>) -> (u64, u64) {
    let Ok(entries) = std::fs::read_dir(cache) else {
        return (0, 0);
    };
    let (mut removed, mut freed) = (0, 0);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let kept = name.strip_suffix(".miyu").is_some_and(|stem| {
            keep.iter().any(|wanted| {
                stem == wanted
                    || stem
                        .strip_prefix(wanted.as_str())
                        .is_some_and(|rest| rest.starts_with('-'))
            })
        });
        if kept {
            continue;
        }
        let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        if std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
            freed += size;
        }
    }
    (removed, freed)
}

#[cfg(test)]
mod tests;
