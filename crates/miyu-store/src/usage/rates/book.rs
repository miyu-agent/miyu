//! 找汇率（施工 UC-1）：缓存里有的直接用，没有的交给取汇率的端口去取。同时缺的并成一次；同一个缺口在取的，后来的跟着
//! 等，不再取一次；最多等一段时间（[`WAIT`]），等不到的这一次换不成，取回来的照样记下、下一次就有。暂定的过期了照用旧的，
//! 在后面重取，不等。端口没装的（测试里造的、核心起来之前）只用缓存。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant};

use super::cache::Cache;
use super::{Day, USD, day_of};

/// 缺的汇率当场去取，最多等多久（核心主会话 2026-10-24 定）。
pub const WAIT: Duration = Duration::from_secs(2);

/// 汇率从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Frankfurter（主源）：开源、不要 key、不要求署名、有历史。
    Frankfurter,
    /// open.er-api.com（兜底）：只有最新一份；免费条款要求署名，用到它时界面注明来源。
    Fallback,
}

impl Source {
    /// 记进缓存、写进协议的名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Frankfurter => "frankfurter",
            Source::Fallback => "open.er-api.com",
        }
    }

    /// 照记下的名字认；不认识的当主源。
    pub(crate) fn parse(text: &str) -> Source {
        match text {
            "open.er-api.com" => Source::Fallback,
            _ => Source::Frankfurter,
        }
    }
}

/// 取到的一个（日子，币种）。
#[derive(Debug, Clone, PartialEq)]
pub struct Fetched {
    /// 哪个 UTC 日子的。
    pub day: Day,
    /// 币种代码。
    pub currency: String,
    /// 1 美元换多少；源说不认识这个币种的是空的。
    pub rate: Option<f64>,
    /// 哪个源。
    pub source: Source,
    /// 定了：那一天已经过完、源交回的就是那一天的。当天的、兜底源的、交回的日子比要的早的是暂定的。
    pub settled: bool,
}

/// 取汇率的端口：上层用 HTTP 实现，核心起来时装上。在别处取，不挡着调的一方；取完把取到的交给 `done`，一个都没取到也要交
/// （空的），等着的才不白等。只交币种和日子，不交任何用量。
pub trait RateSource: Send + Sync {
    /// 取 `wanted` 里的（照日子、币种排好，不带美元），`today` 是这一刻的 UTC 日子（定没定照它判）。
    fn start(
        &self,
        wanted: Vec<(Day, String)>,
        today: Day,
        done: Box<dyn FnOnce(Vec<Fetched>) + Send>,
    );
}

/// 找汇率的：缓存、装上的端口、在取的缺口。一个核心一份。
pub struct RateBook {
    pub(super) cache: Cache,
    source: RwLock<Option<Arc<dyn RateSource>>>,
    /// 在取的缺口。
    flying: Mutex<BTreeSet<(Day, String)>>,
    /// 取回来一批就叫醒等着的。
    landed: Condvar,
}

impl std::fmt::Debug for RateBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RateBook")
    }
}

impl RateBook {
    /// 打开 `path` 的缓存（`state/rates.db`），还没装端口。
    pub fn open(path: &Path) -> RateBook {
        RateBook {
            cache: Cache::open(path),
            source: RwLock::new(None),
            flying: Mutex::new(BTreeSet::new()),
            landed: Condvar::new(),
        }
    }

    /// 装上取汇率的端口，换掉之前装的。
    pub fn install(&self, source: Arc<dyn RateSource>) {
        *self.source.write().unwrap_or_else(PoisonError::into_inner) = Some(source);
    }

    /// 照这一刻 `now`（毫秒）找 `wanted` 里每一个（日子，币种）1 美元换多少，交回找到的和它的源。美元自己是 1。缓存里没有的
    /// 去取，最多等 `wait`；暂定过期了的照用缓存里的、在后面重取。
    pub fn rates(
        self: &Arc<Self>,
        wanted: &BTreeSet<(Day, String)>,
        now: i64,
        wait: Duration,
    ) -> BTreeMap<(Day, String), (f64, Source)> {
        let mut found = BTreeMap::new();
        let mut missing = Vec::new();
        let mut asking = Vec::new();
        for (day, currency) in wanted {
            let key = (*day, currency.clone());
            if currency == USD {
                found.insert(key, (1.0, Source::Frankfurter));
                continue;
            }
            match self.cache.get(*day, currency) {
                Some(cached) => {
                    if let Some(rate) = cached.rate {
                        found.insert(key.clone(), (rate, cached.source));
                    }
                    if !cached.fresh(now) {
                        asking.push(key);
                    }
                }
                None => {
                    missing.push(key.clone());
                    asking.push(key);
                }
            }
        }
        self.ask(asking, now);
        if missing.is_empty() {
            return found;
        }
        self.until_landed(&missing, wait);
        for (day, currency) in missing {
            if let Some(cached) = self.cache.get(day, &currency)
                && let Some(rate) = cached.rate
            {
                found.insert((day, currency), (rate, cached.source));
            }
        }
        found
    }

    /// 把 `asking` 里还没人在取的交给端口：取回来记进缓存（取的时刻照 `now`），从在取的里拿掉，叫醒等着的。
    fn ask(self: &Arc<Self>, asking: Vec<(Day, String)>, now: i64) {
        let source = self
            .source
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let Some(source) = source else {
            return;
        };
        let list: Vec<(Day, String)> = {
            let mut flying = self.flying.lock().unwrap_or_else(PoisonError::into_inner);
            asking
                .into_iter()
                .filter(|key| flying.insert(key.clone()))
                .collect()
        };
        if list.is_empty() {
            return;
        }
        let book = Arc::clone(self);
        let asked = list.clone();
        source.start(
            list,
            day_of(now),
            Box::new(move |fetched| {
                book.cache.put(&fetched, now);
                let mut flying = book.flying.lock().unwrap_or_else(PoisonError::into_inner);
                for key in &asked {
                    flying.remove(key);
                }
                book.landed.notify_all();
            }),
        );
    }

    /// 等到 `missing` 里的都不在取了，最多等 `wait`。
    fn until_landed(&self, missing: &[(Day, String)], wait: Duration) {
        let deadline = Instant::now() + wait;
        let mut flying = self.flying.lock().unwrap_or_else(PoisonError::into_inner);
        while missing.iter().any(|key| flying.contains(key)) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            flying = self
                .landed
                .wait_timeout(flying, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}
