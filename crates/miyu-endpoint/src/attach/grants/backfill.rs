//! 升级后第一次起来，把包以前存的 blob 补登记给它（施工 BG-1补，`docs/construction/BG-1-包存的图重起后读得到.md`「补登记」）：
//! BG-1 之前存的 blob，核心从来没记过是谁存的；表情包这样的包把图只存在核心里、自己的状态目录里只记哈希，升级到带 BG-1 的
//! 核心以后老图照样 `unknown_blob`。
//!
//! 数据根里还没有 `var/lib/blob-grants/` 的那一次起来（升级后第一次，或者全新的数据根）：照装着的每个包，扫它自己的状态目录
//! `var/lib/<编号>/`（`miyu_store::blob::grants::scan`：照字节找哈希，不跟链接、不出目录、有上限），找到的里面存处确实有的
//! 登记给它、落盘，然后把目录建出来，哪怕一个包都没补到。目录在了就不再做。
//!
//! 为什么只做这一次：包以前能把哈希记进自己的状态，说明它当时拿得到；BG-1 以后许可都落盘了，不用再从状态目录认。要是每次
//! 起来都扫，包就能靠往自己目录里写别处看来的哈希给自己开门。
//!
//! 要在拉起扩展之前做完（[`Core::backfill_blob_grants`]）：不然扩展起来就读图，还没补上。「目录在不在」就是「补没补完」：
//! 全扫完以后各包的那一份先写进旁边的暂存目录，再一次改名成 `var/lib/blob-grants/`（`miyu_store::blob::grants::seed`）；中途
//! 被杀的只剩暂存目录，下次起来先删掉它、整个重做。

use std::collections::BTreeSet;

use miyu_kernel::id::ContentHash;
use miyu_store::blob::Blobs;
use miyu_store::blob::grants as saved;
use miyu_store::blob::grants::scan::{Limits, scan};
use miyu_store::root::DataRoot;

use super::{Grants, TARGET};
use crate::Core;

#[cfg(test)]
mod tests;

/// 补登记给一个包的：哪个包、哪几个、落没落盘。
type Seeded = (String, BTreeSet<ContentHash>, bool);

impl Core {
    /// 升级后第一次起来的补登记（施工 BG-1补）：核心起来、拉起扩展（[`Core::start_extensions`]）之前调一次、等它做完。做过的
    /// （目录在了）当场交回。扫描在阻塞线程里。出了什么错都只记一行，不拦核心起来。
    pub async fn backfill_blob_grants(&self) {
        // 清单读得成的才算装着：读不成的（编号撞了核心自己的名字这些）不拉起，`var/lib/<编号>/` 也可能是核心自己的目录。
        let packages: BTreeSet<String> = self
            .system_packages()
            .iter()
            .filter(|found| found.read.is_ok())
            .map(|found| found.id.clone())
            .collect();
        self.blob_grants
            .backfill(packages.into_iter().collect())
            .await;
    }
}

impl Grants {
    /// 照 `packages` 这几个包的状态目录补登记一次（模块说明）。不落盘的（没有数据根）不做。
    pub(super) async fn backfill(&self, packages: Vec<String>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let seeded = match tokio::task::spawn_blocking(move || seed(&root, &packages)).await {
            Ok(seeded) => seeded,
            Err(error) => {
                tracing::error!(target: TARGET, panic = error.is_panic(), "blob grants backfill failed");
                return;
            }
        };
        let mut held = self.lock();
        for (package, hashes, written) in seeded {
            // 没换进去的这一次起来照旧读得到，不算落了盘：下次起来整个重做。
            if written {
                held.saved
                    .entry(package.clone())
                    .or_default()
                    .extend(hashes.iter().cloned());
            }
            held.all.entry(package).or_default().extend(hashes);
        }
    }
}

/// 在阻塞线程里：目录还没有的，扫、认、一下子换进去；交回补登记了的。
fn seed(root: &DataRoot, packages: &[String]) -> Vec<Seeded> {
    match saved::opened(root) {
        Ok(false) => {}
        Ok(true) => return Vec::new(),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "blob grants directory unreadable, backfill skipped");
            return Vec::new();
        }
    }
    let (blobs, limits) = (Blobs::new(root.blobs()), Limits::default());
    let found: Vec<(String, BTreeSet<ContentHash>)> = packages
        .iter()
        .filter_map(|package| Some((package.clone(), noted(root, &blobs, &limits, package)?)))
        .collect();
    // 一下子换进去：没换成的这一次起来照旧读得到，不算落了盘，下次起来整个重做。
    let written = saved::seed(root, &found)
        .inspect_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "blob grants backfill not saved, runs again next start");
        })
        .is_ok();
    let seeded: Vec<Seeded> = found
        .into_iter()
        .map(|(package, hashes)| (package, hashes, written))
        .collect();
    let granted: usize = seeded.iter().map(|(_, hashes, _)| hashes.len()).sum();
    tracing::info!(target: TARGET, packages = packages.len(), granted, "blob grants backfill done");
    seeded
}

/// 包 `package` 的状态目录里记着、存处确实有的哈希；一个都没有的、目录读不了的（记一行）是 `None`。
fn noted(
    root: &DataRoot,
    blobs: &Blobs,
    limits: &Limits,
    package: &str,
) -> Option<BTreeSet<ContentHash>> {
    let scanned = scan(&root.package_state(package), limits)
        .inspect_err(|error| {
            tracing::warn!(target: TARGET, package, error = %error, "blob grants backfill: package state unreadable");
        })
        .ok()?;
    if !scanned.complete() {
        tracing::warn!(
            target: TARGET,
            package,
            files = scanned.files,
            too_large = scanned.too_large,
            too_deep = scanned.too_deep,
            unreadable = scanned.unreadable,
            cut_short = scanned.cut_short,
            "blob grants backfill: package state not fully scanned"
        );
    }
    let found = scanned.found.len();
    // 只留存处确实有的：同 `Blobs::put` 认「已经有了」的办法。
    let stored: BTreeSet<ContentHash> = scanned
        .found
        .into_iter()
        .filter(|hash| blobs.path(hash).is_file())
        .collect();
    if found > 0 {
        tracing::info!(target: TARGET, package, found, granted = stored.len(), "blob grants backfilled");
    }
    (!stored.is_empty()).then_some(stored)
}
