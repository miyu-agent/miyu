//! 扩展不带 `session` 读得到哪些 blob（施工 W-6 三补，`protocol.md` 的 `blob.get` 第 1 条）：存处只有一个，不记的话扩展知道哈希
//! 就能读走任何一个。核心记下每个包拿到过哪些：
//!
//! - 它自己存的：`blob.put`、分块上传 `blob.close` 的回应（[`Grants::grant`]）；
//! - 它经 `package.call` 调别的包拿回来的：回应里出现的哈希，答的那个包自己拿得到的才转给它（[`Grants::pass`]）。只认答的包
//!   自己拿得到的：不然一个包把别处看来的哈希写进回应，就能替调它的包开门。
//! - 别人调它时交过来的（施工 BUS-2，设计 67 第四节）：调的扩展参数里写的、它自己拿得到的哈希（[`Grants::pass`] 反过来用）；
//!   人的连接交的都算（[`Grants::hand`]）。语音把录音交给识别、把音频交给播放靠这一条。
//!
//! 它自己存的另落盘（施工 BG-1，`miyu_store::blob::grants`：`var/lib/blob-grants/<包>`，一行一个哈希），核心起来时读回
//! （[`Grants::load`]）：表情包这样的包把图只存在 blob 里、自己只记哈希，重起以后手里没有内容，没法再存一次。后两种只记在内存
//! 里，核心重起就没了：那是一次调用里临时给的。包卸了，它的一份扔掉、文件删掉（[`Grants::forget`]），装回来从空的开始。

use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, MutexGuard, PoisonError};

use miyu_kernel::id::ContentHash;
use miyu_store::blob::grants as saved;
use miyu_store::root::DataRoot;
use serde_json::Value;

const TARGET: &str = "miyu::endpoint";

/// 各个包拿得到的 blob。
#[derive(Debug, Default)]
pub(crate) struct Grants {
    held: Mutex<Held>,
    /// 自己存的落在哪个数据根（施工 BG-1）；没有的只记在内存里（单元测试里不落盘的那几条）。
    root: Option<DataRoot>,
}

/// 记着的两张表，包的编号到哈希。
#[derive(Debug, Default)]
struct Held {
    /// 拿得到的：三种来处都在里面。
    all: HashMap<String, BTreeSet<ContentHash>>,
    /// 其中自己存的、落了盘（或者正在落）的（施工 BG-1）：同一个哈希只写一行。
    saved: HashMap<String, BTreeSet<ContentHash>>,
}

impl Grants {
    /// 核心起来时：把数据根 `root` 里各个包自己存过的读回来（施工 BG-1），之后存的接着往里记。读不了的记一行、从空的开始；
    /// 读不懂的行跳过，一份记一行。
    pub(crate) fn load(root: &DataRoot) -> Grants {
        let loaded = saved::load(root).unwrap_or_else(|error| {
            tracing::warn!(target: TARGET, error = %error, "blob grants unreadable, starting empty");
            saved::Loaded::default()
        });
        for (package, lines) in &loaded.bad {
            tracing::warn!(target: TARGET, package = package.as_str(), lines, "blob grants: bad lines skipped");
        }
        for (package, error) in &loaded.unreadable {
            tracing::warn!(target: TARGET, package = package.as_str(), error = %error, "blob grants of a package unreadable");
        }
        Grants {
            held: Mutex::new(Held {
                all: loaded.held.clone(),
                saved: loaded.held,
            }),
            root: Some(root.clone()),
        }
    }

    /// 包 `package` 自己存了 `hash`：它拿得到，头一次存的另落盘（施工 BG-1）。写不进的记一行，存照算成：这一次起来照旧读得到，
    /// 下一次再存再试着写。
    pub(crate) async fn grant(&self, package: &str, hash: &ContentHash) {
        let fresh = {
            let mut held = self.lock();
            let own = |held: &mut HashMap<String, BTreeSet<ContentHash>>| {
                held.entry(package.to_string())
                    .or_default()
                    .insert(hash.clone())
            };
            own(&mut held.all);
            own(&mut held.saved)
        };
        let Some(root) = self.root.clone().filter(|_| fresh) else {
            return;
        };
        let (name, stored) = (package.to_string(), hash.clone());
        let written = tokio::task::spawn_blocking(move || saved::append(&root, &name, &stored))
            .await
            .unwrap_or_else(|error| Err(std::io::Error::other(error)));
        if let Err(error) = written {
            tracing::warn!(target: TARGET, package, error = %error, "blob grant not saved");
            if let Some(own) = self.lock().saved.get_mut(package) {
                own.remove(hash);
            }
        }
    }

    /// 包 `package` 拿不拿得到 `hash`。
    pub(crate) fn holds(&self, package: &str, hash: &ContentHash) -> bool {
        self.lock()
            .all
            .get(package)
            .is_some_and(|held| held.contains(hash))
    }

    /// 包 `from` 答 `to` 的回应 `answer`：里面哪个字符串是 `from` 拿得到的哈希，`to` 也拿得到（施工 SU-4 下起存处只有一个，
    /// 只记许可、不用拷）。只在内存里，不落盘。
    pub(crate) fn pass(&self, from: &str, to: &str, answer: &Value) {
        let mut found = Vec::new();
        hashes(answer, &mut found);
        if found.is_empty() {
            return;
        }
        let mut held = self.lock();
        let Some(own) = held.all.get(from) else {
            return;
        };
        let passed: Vec<ContentHash> = found
            .into_iter()
            .filter(|hash| own.contains(hash))
            .collect();
        held.all.entry(to.to_string()).or_default().extend(passed);
    }

    /// 人的连接经总线、`package.call` 交给包 `to` 的参数 `given`（施工 BUS-2）：里面合哈希写法的都交给它。人本来什么都读得到，
    /// 交出去的哈希存处没有的，读的时候照样读不到。只在内存里，不落盘。
    pub(crate) fn hand(&self, to: &str, given: &Value) {
        let mut found = Vec::new();
        hashes(given, &mut found);
        if found.is_empty() {
            return;
        }
        self.lock()
            .all
            .entry(to.to_string())
            .or_default()
            .extend(found);
    }

    /// 包 `package` 卸了：它的一份扔掉，落盘的那一份删掉（施工 BG-1）；删不掉的记一行，卸照算成。
    pub(crate) async fn forget(&self, package: &str) {
        {
            let mut held = self.lock();
            held.all.remove(package);
            held.saved.remove(package);
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        let name = package.to_string();
        let removed = tokio::task::spawn_blocking(move || saved::forget(&root, &name))
            .await
            .unwrap_or_else(|error| Err(std::io::Error::other(error)));
        if let Err(error) = removed {
            tracing::warn!(target: TARGET, package, error = %error, "blob grants of a removed package not deleted");
        }
    }

    fn lock(&self) -> MutexGuard<'_, Held> {
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
