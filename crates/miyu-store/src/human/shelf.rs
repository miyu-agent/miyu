//! 读好的字放一阵（施工 V-30）：一种语言读一次，文件没变的大家共用，核心安静下来时放掉。
//!
//! 给人看的字只从资源目录读（内核那一份、`software/` 下每个出厂包一份）。原来每一处要字的都从头读：读几十份 JSON、每一句
//! 解成模板、补单数，用完扔掉。头一连上，配置、给人看的字、软件包列表、人格、预设、订阅推的每一样各读一遍，一次冷启动读了
//! 41 遍，占核心冷启动 CPU 的四成。现在读好的照语言放在 [`Shelf`] 里，[`Human::load`] 先看这里；放着的一份交出去是拷一份
//! （每一句说法共用，不重新解）。
//!
//! 放着的什么时候不算数：读的时候记下每一处的这种语言、英文两份文件的长短和修改时刻（没有的记「没有」），连同有哪几处
//! （[`Stamp`]）；再要的时候照样看一遍（几十次 `stat`、列一次 `software/`），有一样不同就重读。改了资源，核心不重启下一次
//! 就是新的（施工 W-1 定的）。一样长、修改时刻也一样的改动看不出来：同 V-24 核对图片的取舍。
//!
//! 放多久：核心一阵请求过去、安静下来时（`miyu-core` 的 `settle`）连同还堆一起放掉（[`ResourceRoot::forget_words`]），
//! 核心空闲时不多占。读不懂的不放，下一次照旧再读、照旧报错。

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use super::{FALLBACK, Human, HumanError};
use crate::resources::ResourceRoot;

/// 读好的字，照语言放。拷出来的几份共用同一个架子：资源目录拷到哪，读好的字跟到哪。
#[derive(Clone, Default)]
pub struct Shelf {
    held: Arc<Mutex<BTreeMap<String, Held>>>,
}

/// 放着的一份和读它的时候那几份文件的样子。
struct Held {
    human: Arc<Human>,
    stamp: Stamp,
}

/// 一种语言读自哪几份文件，读的时候各是什么样：每一处的这种语言、英文两份，照先后；没有的是 `None`。
#[derive(PartialEq, Eq)]
struct Stamp(Vec<(PathBuf, Option<(u64, SystemTime)>)>);

impl Stamp {
    fn of(places: &[(PathBuf, String)], language: &str) -> Stamp {
        let languages: &[&str] = if language == FALLBACK {
            &[FALLBACK]
        } else {
            &[language, FALLBACK]
        };
        let mut files = Vec::with_capacity(places.len() * languages.len());
        for (dir, _) in places {
            for language in languages {
                let file = dir.join("human").join(format!("{language}.json"));
                let seen = std::fs::metadata(&file)
                    .ok()
                    .and_then(|meta| Some((meta.len(), meta.modified().ok()?)));
                files.push((file, seen));
            }
        }
        Stamp(files)
    }
}

impl Shelf {
    /// `language` 那一份：放着的、`places` 里那几份文件都没变的，交回放着的；不然照 `read` 读，读成了放上。先看文件
    /// 再读：读的当中又改了的，下一次看出来重读。读的时候拿着锁：几处同时要同一种语言的，只读一遍，别的等它。
    pub(crate) fn get(
        &self,
        language: &str,
        places: &[(PathBuf, String)],
        read: impl FnOnce() -> Result<Human, HumanError>,
    ) -> Result<Arc<Human>, HumanError> {
        let stamp = Stamp::of(places, language);
        let mut held = self.lock();
        if let Some(found) = held.get(language).filter(|found| found.stamp == stamp) {
            return Ok(Arc::clone(&found.human));
        }
        let human = Arc::new(read()?);
        let kept = Held {
            human: Arc::clone(&human),
            stamp,
        };
        held.insert(language.to_string(), kept);
        Ok(human)
    }

    /// 放掉放着的全部；交回原来有没有。
    fn clear(&self) -> bool {
        let taken = std::mem::take(&mut *self.lock());
        !taken.is_empty()
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, Held>> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 架子不算资源目录的一部分：两个资源目录一样不一样，只看目录。
impl PartialEq for Shelf {
    fn eq(&self, _: &Shelf) -> bool {
        true
    }
}

impl Eq for Shelf {}

impl fmt::Debug for Shelf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let held: Vec<String> = self.lock().keys().cloned().collect();
        f.debug_struct("Shelf").field("held", &held).finish()
    }
}

impl ResourceRoot {
    /// 放掉读好放着的字（施工 V-30）：核心安静下来时调，下一次要字的再读。交回原来有没有放着的。
    pub fn forget_words(&self) -> bool {
        self.shelf.clear()
    }
}

#[cfg(test)]
mod tests;
