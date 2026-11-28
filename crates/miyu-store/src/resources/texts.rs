//! 读好的给模型看的字放一阵（施工 PERF-5）：同一份字文件没变的只读一遍，核心安静下来时放掉。
//!
//! 造会话、预演新会话的上下文（`preset.list` 每个预设一遍、`session.context` 不写会话的一遍）每一遍都要把随核心附带的
//! 一百多份字从头读一遍（[`ResourceRoot::read`]），每一份还要看一眼你那一层有没有同名的覆盖（`overlay.rs`）。终端界面打开
//! 一次核心预演 6 遍，开文件三千多次，一半多是这些字和覆盖（覆盖多半没有，开了也是「不存在」）。
//!
//! 放着的什么时候不算数，同给人看的字（施工 V-30，`human/shelf.rs`）：读的时候记下出厂那一份和每一层覆盖的长短、修改时刻
//! （没有的记「没有」）；再要的时候照样看一遍（`stat`，不开文件），有一样不同就重读。所以改了资源、加了改了删了覆盖，
//! 核心不重启下一次就是新的；一样长、修改时刻也一样的改动看不出来，同 V-30 的取舍。先看文件再读：读的当中又改了的，下一次
//! 看出来重读。看不清的（修改时刻拿不到、看的时候出了「不存在」以外的错）不放，照旧每次读。读不出来的不放，下一次照旧报错。
//!
//! 放多久：同给人看的字，核心一阵请求过去、安静下来时一起放掉（[`ResourceRoot::forget_words`]）。

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use super::ResourceRoot;

/// 读好的字：照「出厂那一份的位置、几层覆盖」放。拷出来的资源目录共用同一个架子，压了覆盖的（`overlaid`）也是：几层
/// 覆盖记在钥匙里，压法不同的各放各的。
#[derive(Clone, Default)]
pub(crate) struct Texts {
    held: Arc<Mutex<BTreeMap<Key, Held>>>,
    /// 照文件读了几次（测试看：放着的不再读）。
    #[cfg(any(test, feature = "testkit"))]
    reads: Arc<std::sync::atomic::AtomicUsize>,
}

/// 出厂那一份的位置、要看的几层覆盖的位置（从上往下）。
type Key = (PathBuf, Vec<PathBuf>);

/// 放着的一份字和读它的时候那几份文件的样子。
struct Held {
    text: String,
    stamp: Stamp,
}

/// 读的时候那几份文件各是什么样：长短和修改时刻，没有的是 `None`。照钥匙的先后：出厂的、每一层覆盖。
#[derive(PartialEq, Eq)]
struct Stamp(Vec<Option<(u64, SystemTime)>>);

impl Stamp {
    /// 看一遍 `shipped`、`overlays`（只 `stat`，不开文件）。有一份看不清的交回没有：不放、不认放着的。
    fn of(shipped: &Path, overlays: &[PathBuf]) -> Option<Stamp> {
        std::iter::once(shipped)
            .chain(overlays.iter().map(PathBuf::as_path))
            .map(seen)
            .collect::<Option<Vec<_>>>()
            .map(Stamp)
    }
}

/// 一份文件的样子：`Some(None)` 是没有；看的时候出了别的错、修改时刻拿不到的是看不清（`None`）。
fn seen(path: &Path) -> Option<Option<(u64, SystemTime)>> {
    match std::fs::metadata(path) {
        Ok(meta) => Some(Some((meta.len(), meta.modified().ok()?))),
        Err(error) if is_absent(&error) => Some(None),
        Err(_) => None,
    }
}

/// 「没有这份」：不存在，或者路径上有一截是文件（覆盖那一层里同名的目录被一个文件占着）。
fn is_absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

impl Texts {
    /// `shipped` 那一份字（看 `overlays` 那几层覆盖）：放着的、那几份文件都没变的，交回放着的；不然照 `read` 读，读成了
    /// 放上。先看文件再读，读的时候不拿着锁。
    pub(crate) fn get<E>(
        &self,
        shipped: &Path,
        overlays: &[PathBuf],
        read: impl FnOnce() -> Result<String, E>,
    ) -> Result<String, E> {
        let stamp = Stamp::of(shipped, overlays);
        let key = (shipped.to_path_buf(), overlays.to_vec());
        if let Some(stamp) = &stamp
            && let Some(found) = self.lock().get(&key).filter(|found| found.stamp == *stamp)
        {
            return Ok(found.text.clone());
        }
        #[cfg(any(test, feature = "testkit"))]
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let text = read()?;
        if let Some(stamp) = stamp {
            let kept = Held {
                text: text.clone(),
                stamp,
            };
            self.lock().insert(key, kept);
        }
        Ok(text)
    }

    /// 放掉放着的全部；交回原来有没有。
    pub(crate) fn clear(&self) -> bool {
        let taken = std::mem::take(&mut *self.lock());
        !taken.is_empty()
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<Key, Held>> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 架子不算资源目录的一部分：两个资源目录一样不一样，只看目录。
impl PartialEq for Texts {
    fn eq(&self, _: &Texts) -> bool {
        true
    }
}

impl Eq for Texts {}

impl fmt::Debug for Texts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Texts({} held)", self.lock().len())
    }
}

impl ResourceRoot {
    /// 放掉读好放着的给模型看的字；交回原来有没有。[`ResourceRoot::forget_words`] 连同给人看的字一起放。
    pub(crate) fn forget_texts(&self) -> bool {
        self.texts.clear()
    }

    /// 给模型看的字照文件读了几次，拷出来的、压了覆盖的几份一起数（测试看：文件没变的不再读）。
    #[cfg(any(test, feature = "testkit"))]
    #[must_use]
    pub fn texts_read(&self) -> usize {
        self.texts.reads.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests;
