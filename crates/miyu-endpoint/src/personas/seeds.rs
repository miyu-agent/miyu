//! 人格头像的候选主色（施工 P-6 补，`docs/blueprint/personas.md`「主题色、背景图」第 4 条）：照 `miyu-seeds` 从头像挑最多 4 个，
//! `persona.list`、`persona.get` 的 `seeds` 给它们，打分最高的排第一。2026-10-14 项目主人定头像取色归核心：两个头拿到的一样，
//! 选中哪个由头写进人格的 `seed`。
//!
//! 同一张图只算一次：照图的字节的哈希记在这个进程里，换了图哈希就变；记多了整个清掉重来。

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

use miyu_kernel::id::ContentHash;
use miyu_store::personas::Found;

/// 最多记多少张图的候选：人格就那么几个，超了整个清掉。
const KEEP: usize = 64;

/// 记下来的：图的哈希 → 候选。
static PICKED: LazyLock<Mutex<BTreeMap<String, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// 叠好的人格的头像挑出的候选，小写的 `#rrggbb`：没有头像、读不了、解不开的是 `None`，全透明的是空的。读盘、解图，在阻塞
/// 线程里调。
pub(crate) fn of(found: &Found) -> Option<Vec<String>> {
    let bytes = std::fs::read(found.avatar.as_deref()?).ok()?;
    let key = ContentHash::of(&bytes).hex().to_string();
    if let Some(seeds) = picked().get(&key) {
        return Some(seeds.clone());
    }
    let seeds: Vec<String> = miyu_seeds::from_image(&bytes)?
        .into_iter()
        .map(miyu_seeds::Seed::hex)
        .collect();
    let mut picked = picked();
    if picked.len() >= KEEP {
        picked.clear();
    }
    picked.insert(key, seeds.clone());
    Some(seeds)
}

/// 记下来的那一张表；别的线程拿着锁时崩了也照用（里面只是缓存）。
fn picked() -> std::sync::MutexGuard<'static, BTreeMap<String, Vec<String>>> {
    PICKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
