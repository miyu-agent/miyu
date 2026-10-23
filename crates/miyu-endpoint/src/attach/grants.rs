//! 扩展不带 `session` 读得到哪些 blob（施工 W-6 三补，`protocol.md` 的 `blob.get` 第 1 条）：以前不带 `session` 的一律照管理员名下
//! 给，扩展知道哈希就能读走管理员名下任何一个。现在核心记下每个包拿到过哪些：
//!
//! - 它自己存的：`blob.put`、分块上传 `blob.close` 的回应（[`Grants::grant`]）；
//! - 它经 `package.call` 调别的包拿回来的：回应里出现的哈希，答的那个包自己拿得到的才转给它（[`Grants::pass`]）。只认答的包
//!   自己拿得到的：不然一个包把别处看来的哈希写进回应，就能替调它的包开门。
//!
//! 只记在内存里：核心重起以后从空的开始，扩展要读回以前存的，再存一次（同样的内容是同一个哈希）。包卸了，它的一份扔掉
//! （[`Grants::forget`]），装回来从空的开始。

use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, PoisonError};

use miyu_kernel::id::ContentHash;
use serde_json::Value;

/// 各个包拿得到的 blob。
#[derive(Debug, Default)]
pub(crate) struct Grants {
    held: Mutex<HashMap<String, BTreeSet<ContentHash>>>,
}

impl Grants {
    /// 包 `package` 拿得到 `hash`。
    pub(crate) fn grant(&self, package: &str, hash: &ContentHash) {
        self.lock()
            .entry(package.to_string())
            .or_default()
            .insert(hash.clone());
    }

    /// 包 `package` 拿不拿得到 `hash`。
    pub(crate) fn holds(&self, package: &str, hash: &ContentHash) -> bool {
        self.lock()
            .get(package)
            .is_some_and(|held| held.contains(hash))
    }

    /// 包 `from` 答 `to` 的回应 `answer`：里面哪个字符串是 `from` 拿得到的哈希，`to` 也拿得到。交回转过去的那几个：两个包
    /// 各是自己的账号（施工 Y-5 中上），调的一方照它把 blob 拷进 `to` 名下。
    pub(crate) fn pass(&self, from: &str, to: &str, answer: &Value) -> Vec<ContentHash> {
        let mut found = Vec::new();
        hashes(answer, &mut found);
        if found.is_empty() {
            return Vec::new();
        }
        let mut held = self.lock();
        let passed: Vec<ContentHash> = match held.get(from) {
            Some(own) => found
                .into_iter()
                .filter(|hash| own.contains(hash))
                .collect(),
            None => return Vec::new(),
        };
        held.entry(to.to_string())
            .or_default()
            .extend(passed.iter().cloned());
        passed
    }

    /// 包 `package` 卸了：它的一份扔掉。
    pub(crate) fn forget(&self, package: &str) {
        self.lock().remove(package);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, BTreeSet<ContentHash>>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// `value` 里合哈希写法的字符串，照先后。
fn hashes(value: &Value, found: &mut Vec<ContentHash>) {
    match value {
        Value::String(text) => found.extend(ContentHash::parse(text).ok()),
        Value::Array(items) => items.iter().for_each(|item| hashes(item, found)),
        Value::Object(fields) => fields.values().for_each(|field| hashes(field, found)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests;
