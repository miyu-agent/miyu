//! 目录不常驻（施工 V-25 下，`docs/blueprint/models.md`「怎么走」第二条第 2 条）：用到的模型第一次从整份目录查一次，答
//! （`miyu_models::catalog::Recalled`）记下来、存进 `state/models/catalog.json`，以后每一轮、核心重启都只照记下的答。整份
//! models.dev 目录平时不在内存里，只在这几种时候临时读、用完放：
//!
//! 1. 遇到没记过的输入（新配的模型、新供应商、会话记着的配置里已经没写的模型）：[`ModelData::with`] 先只照记下的合一遍，
//!    说不出来的（`Recalling::missed` 立了）整遍作废，借整份目录重合一遍、记下、写盘。
//! 2. 要整份目录的地方（`model.list` 列那一家目录里的全部模型、第一次接入、搜目录、常用的几家、`provider.test` 照目录挑模型、
//!    图标）：[`ModelData::hold`] 临时读一份，[`ModelData::with_holding`] 借着它合；临时问的不记。
//! 3. 后台拉到新目录（[`ModelData::replace_catalog`]）、临时读回来的版本和记下的不一样（缓存被别的核心换过）：照新目录把
//!    记下的键全部重算（`Recalled::rebuilt`），原子地换上新的一份、写盘。
//!
//! 锁的先后（只朝一个方向拿，不会互相等死）：补算的那把 `missing` → 读目录的那把 `slot` → 写盘的那把 `saving` → 记下的那份
//! 自己的锁。用出来的、列表的那把（`observed`）里只合资料、不读目录：读整份目录这种慢活都在 `slot` 里做，拿着 `observed` 的
//! 从不去拿前三把。

use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::time::Instant;

use miyu_config::merge::Resolved;
use miyu_models::Knowledge;
use miyu_models::catalog::{CatalogLookup, CatalogSource, Loaded, Recalled, Recalling};
use miyu_models::facts::facts;
use miyu_models::provider;
use miyu_models::reference::{Reference, named};
use miyu_models::settings::UseSettings;

use crate::TARGET;

use super::{ModelData, Observed};

/// 记下的那份存在 `state/models/` 下的这个文件里。
pub const RECALL_FILE: &str = "catalog.json";

/// 临时读一份整份目录（核心交进来，`miyu-core` 的 `models/recall.rs`）：读的是这一刻排第一、读得了的那一份，连同它的版本串
/// （目录的指纹、认原厂的表的哈希，和记下的那份的 `of` 同一个算法）。都读不了的没有。慢：在阻塞线程里、或者只在头一回调。
pub type CatalogReader = Arc<dyn Fn() -> Option<(Loaded, String)> + Send + Sync>;

/// 在用的目录：常驻的整份，或者记下的那份。
#[derive(Debug, Clone)]
pub(super) enum Held {
    /// 常驻：整份目录在内存里；都读不了的没有。没有缓存目录、不知道程序在哪的（测试、`models::routes()`）一直走这条。
    Full(Option<Arc<Loaded>>),
    /// 不常驻：记下的那份。换版本时整份换成新的（换 `Arc`，不改旧的里面），借着旧的合到一半的照旧合完。
    Recalled(Arc<Mutex<Recalled>>),
}

/// 不常驻时要的几样：怎么读整份目录、临时读来的那一份、几把锁。
#[derive(Default)]
pub(super) struct Recall {
    /// 怎么临时读整份目录：不常驻时才有。
    reader: OnceLock<CatalogReader>,
    /// 临时读来的那一份和它的版本串：只留弱引用，借的人都放了就放掉。同时要的（几个 `model.list` 一起来）借同一份，不各读
    /// 一次。读的时候拿着这把锁，后来的等它读完再借。
    slot: Mutex<Option<(Weak<Loaded>, String)>>,
    /// 补算没记过的输入时拿着：同时遇到同一个没记过的输入的，后来的等前一个记下了再照记下的答，整份目录只读一次。
    missing: Mutex<()>,
    /// 写盘时拿着：每次写的都是这一刻最新的那份，几次写不会把新的盖成旧的。
    saving: Mutex<()>,
}

impl fmt::Debug for Recall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Recall")
            .field("reader", &self.reader.get().is_some())
            .finish_non_exhaustive()
    }
}

/// 借着的整份目录（[`ModelData::hold`]）：常驻的就是那一份；不常驻的是临时读来的，连同读的那一刻对得上它的记下的那份。放掉
/// 它，临时读来的目录没人借了就放掉（一阵请求过去以后核心把空着的堆还给系统）。
#[derive(Debug, Clone, Default)]
pub struct Holding {
    loaded: Option<Arc<Loaded>>,
    recalled: Option<Arc<Mutex<Recalled>>>,
}

impl Holding {
    /// 整份目录；还没读完、都读不了的没有。
    pub fn catalog(&self) -> Option<&Loaded> {
        self.loaded.as_deref()
    }

    /// 不借了，交回整份目录（借着它的引用）。
    pub fn into_catalog(self) -> Option<Arc<Loaded>> {
        self.loaded
    }

    /// 在用的是哪一份、什么时候拉的（`model.list` 的 `catalog`）：不常驻的照记下的那份（临时读不到整份目录时也说得出来，和
    /// 常驻时一样），常驻的照整份目录。都没有的没有。
    pub fn meta(&self) -> Option<(CatalogSource, String)> {
        match (&self.recalled, &self.loaded) {
            (Some(recalled), _) => {
                let recalled = lock(recalled);
                Some((recalled.source(), recalled.fetched().to_string()))
            }
            (None, Some(loaded)) => Some((loaded.source, loaded.fetched.clone())),
            (None, None) => None,
        }
    }
}

impl ModelData {
    /// 起来时不常驻（施工 V-25 下）：换上记下的那份（版本已经验过，或者刚照整份目录记好）、读好的用出来的、供应商的列表，以后
    /// 没记过的照 `reader` 临时读整份目录。写一遍盘，等着的都放行（[`ModelData::wait`]）。
    pub fn recalled(&self, recalled: Recalled, mut observed: Observed, reader: CatalogReader) {
        // 只装一次：核心起来时装上，以后不换。
        if self.recall.reader.set(reader).is_err() {
            tracing::debug!(target: TARGET, "catalog reader already set");
        }
        *self.pointers() = std::mem::take(&mut observed.pointers);
        *self.lock() = observed;
        // 先写盘再放行：等着的一放行就可能有人去读这个文件。
        self.write_recall(&recalled);
        self.catalog
            .send_replace(Some(Held::Recalled(Arc::new(Mutex::new(recalled)))));
    }

    /// 照整份目录 `loaded` 把配置 `resolved` 里写过的都查一遍、记进 `recalled`（起来时版本对不上、没有记下的那份时）：每一家
    /// 配好的供应商，和它在配置里写过的模型（手写了资料的、用途、池里点名的、算向量的）。会话记着的、配置里已经没写的不在
    /// 这里：第一次用到时补（[`ModelData::with`]）。
    pub fn fill(&self, recalled: Recalled, loaded: &Loaded, resolved: &Resolved) -> Recalled {
        let cell = Mutex::new(recalled);
        let lookup = CatalogLookup::Recalled(Recalling {
            recalled: &cell,
            catalog: Some(loaded),
            remember: true,
            missed: None,
        });
        self.run(Some(loaded), Some(lookup), |knowledge| {
            ask_configured(resolved, knowledge);
        });
        cell.into_inner().unwrap_or_else(PoisonError::into_inner)
    }

    /// 配置换了（施工 V-25 下）：把配置里写过的都照记下的问一遍，新写的没记过就在这里临时读整份目录记下——在阻塞线程里调，
    /// 路由下一轮问到时就不用在 tokio 的线程上读了。常驻的什么都不做。
    pub fn remember_configured(&self, resolved: &Resolved) {
        if self.current_recalled().is_some() {
            self.asking(true, |knowledge| ask_configured(resolved, knowledge));
        }
    }

    /// 借一份整份目录（施工 V-25 下）：常驻的直接给；不常驻的临时读一份（同时有人借着的借同一份），读回来的版本和记下的那份
    /// 不一样的先照它重算记下的、换上新的一份。还没读完的、都读不了的借不到目录。慢：不常驻时要读盘，异步的地方在阻塞线程里
    /// 调（[`ModelData::held`]）。
    pub fn hold(&self) -> Holding {
        let held = self.catalog.borrow().clone();
        match held {
            None => Holding::default(),
            Some(Held::Full(loaded)) => Holding {
                loaded,
                recalled: None,
            },
            Some(Held::Recalled(recalled)) => self.hold_recalled(recalled),
        }
    }

    /// 同 [`ModelData::hold`]，在阻塞线程里读。
    pub async fn held(self: &Arc<Self>) -> Holding {
        let data = Arc::clone(self);
        tokio::task::spawn_blocking(move || data.hold())
            .await
            .unwrap_or_else(|error| {
                tracing::error!(target: TARGET, panic = error.is_panic(), "catalog read panicked");
                Holding::default()
            })
    }

    /// 借着 `holding` 的整份目录做一件事：[`Knowledge::catalog`] 是它，合资料照记下的答、没记过的借它现算，`remember` 为真时
    /// 记下、写盘。要整份目录的地方用它（`model.list`、第一次接入、搜目录、`provider.test` 照目录挑模型）。锁同
    /// [`ModelData::with`]。
    pub fn with_holding<R>(
        &self,
        holding: &Holding,
        remember: bool,
        work: impl FnOnce(&Knowledge<'_>) -> R,
    ) -> R {
        let catalog = holding.loaded.as_deref();
        let Some(recalled) = &holding.recalled else {
            return self.run(catalog, catalog.map(CatalogLookup::Full), work);
        };
        let before = lock(recalled).len();
        let lookup = CatalogLookup::Recalled(Recalling {
            recalled,
            catalog,
            remember,
            missed: None,
        });
        let answer = self.run(catalog, Some(lookup), work);
        if remember && lock(recalled).len() != before {
            self.save(recalled);
        }
        answer
    }

    /// 合资料：常驻的拿整份目录现算；不常驻的先只照记下的合一遍，有说不出来的（没记过）整遍作废，拿着补算的锁再照记下的问一遍
    /// （等锁时别人可能已经记下了），还说不出来的临时读整份目录、重合一遍，`remember` 为真时记下、写盘。所以 `work` 可能做两
    /// 遍，作废的那一遍看到的是「没有目录」时的答（认不出、用不了）：只放查资料的纯活，别在里面改东西、等东西，也别断言
    /// 答的一定怎样（把答交出来，在外面看）。
    ///
    /// 遇到没记过的输入，这一次在调的线程上同步读盘（几毫秒到几十毫秒，只在头一回）：路由在 tokio 的线程上同步问资料，改成
    /// 异步要动每一个问的地方；配置里新写的由 [`ModelData::remember_configured`] 先在阻塞线程里补齐，同步读只剩配置里没写、
    /// 会话记着的那种。
    pub(super) fn asking<R>(&self, remember: bool, mut work: impl FnMut(&Knowledge<'_>) -> R) -> R {
        let held = self.catalog.borrow().clone();
        let recalled = match held {
            None => return self.run(None, None, work),
            Some(Held::Full(loaded)) => {
                let catalog = loaded.as_deref();
                return self.run(catalog, catalog.map(CatalogLookup::Full), work);
            }
            Some(Held::Recalled(recalled)) => recalled,
        };
        if let Some(answer) = self.recalling_only(&recalled, &mut work) {
            return answer;
        }
        let _missing = lock(&self.recall.missing);
        let recalled = self.current_recalled().unwrap_or(recalled);
        if let Some(answer) = self.recalling_only(&recalled, &mut work) {
            return answer;
        }
        let started = Instant::now();
        let holding = self.hold();
        tracing::debug!(target: TARGET, ms = started.elapsed().as_millis(), remember, "catalog read for an input not recalled");
        self.with_holding(&holding, remember, work)
    }

    /// 只照记下的合一遍：有说不出来的没有（这一遍作废）。
    fn recalling_only<R>(
        &self,
        recalled: &Mutex<Recalled>,
        work: &mut impl FnMut(&Knowledge<'_>) -> R,
    ) -> Option<R> {
        let missed = AtomicBool::new(false);
        let lookup = CatalogLookup::Recalled(Recalling {
            recalled,
            catalog: None,
            remember: false,
            missed: Some(&missed),
        });
        let answer = self.run(None, Some(lookup), |knowledge| work(knowledge));
        (!missed.load(Ordering::Relaxed)).then_some(answer)
    }

    /// 拿着用出来的、列表的锁合资料，做完就放开。
    pub(super) fn run<R>(
        &self,
        catalog: Option<&Loaded>,
        lookup: Option<CatalogLookup<'_>>,
        work: impl FnOnce(&Knowledge<'_>) -> R,
    ) -> R {
        let observed = self.lock();
        work(&Knowledge {
            profiles: &self.profiles,
            vendors: &self.vendors,
            catalog,
            lookup,
            learned: &observed.learned,
            lists: &observed.lists,
        })
    }

    /// 不常驻时的那份记下的；常驻的、还没读完的没有。
    fn current_recalled(&self) -> Option<Arc<Mutex<Recalled>>> {
        match &*self.catalog.borrow() {
            Some(Held::Recalled(recalled)) => Some(Arc::clone(recalled)),
            _ => None,
        }
    }

    /// 不常驻时借整份目录：有人借着的借同一份，没有的照 `reader` 读；读回来的版本和这一刻记下的不一样的，照它重算、换上。
    fn hold_recalled(&self, recalled: Arc<Mutex<Recalled>>) -> Holding {
        let mut slot = lock(&self.recall.slot);
        let lent = slot
            .as_ref()
            .and_then(|(loaded, of)| Some((loaded.upgrade()?, of.clone())));
        let (loaded, of) = match lent {
            Some(lent) => lent,
            None => {
                let read = self.recall.reader.get().and_then(|reader| reader());
                let Some((loaded, of)) = read else {
                    // 都读不了（读的时候记过 `WARN`）：没记过的说不出来，当没有目录。
                    return Holding {
                        loaded: None,
                        recalled: Some(recalled),
                    };
                };
                let loaded = Arc::new(loaded);
                *slot = Some((Arc::downgrade(&loaded), of.clone()));
                (loaded, of)
            }
        };
        // 等 `slot` 的时候后台可能已经换过一份：照这一刻的比。
        let current = self.current_recalled().unwrap_or(recalled);
        let stale = lock(&current).of() != of;
        let recalled = match stale {
            true => self.swap(&current, &loaded, &of, "changed"),
            false => current,
        };
        Holding {
            loaded: Some(loaded),
            recalled: Some(recalled),
        }
    }

    /// 照新目录 `loaded`（版本串 `of`）把 `current` 记下的键全部重算，原子地换上新的一份、写盘，记一行
    /// `INFO catalog recall rebuilt`。调的一方拿着 `slot`。
    pub(super) fn swap(
        &self,
        current: &Mutex<Recalled>,
        loaded: &Loaded,
        of: &str,
        why: &'static str,
    ) -> Arc<Mutex<Recalled>> {
        let started = Instant::now();
        let rebuilt = lock(current).rebuilt(loaded, &self.vendors, of);
        let (providers, models) = rebuilt.len();
        let rebuilt = Arc::new(Mutex::new(rebuilt));
        self.catalog
            .send_replace(Some(Held::Recalled(Arc::clone(&rebuilt))));
        tracing::info!(
            target: TARGET,
            why,
            source = loaded.source.as_str(),
            fetched = loaded.fetched.as_str(),
            providers,
            models,
            ms = started.elapsed().as_millis(),
            "catalog recall rebuilt"
        );
        self.save(&rebuilt);
        rebuilt
    }

    /// 后台拉到新目录、不常驻时：拿着 `slot` 换（和临时读回来时的换不会一起做），新目录借给这一刻正要借的，没人借了就放掉。
    pub(super) fn swap_refreshed(&self, current: &Mutex<Recalled>, loaded: Loaded, of: &str) {
        let mut slot = lock(&self.recall.slot);
        let loaded = Arc::new(loaded);
        self.swap(current, &loaded, of, "refresh");
        *slot = Some((Arc::downgrade(&loaded), of.to_string()));
    }

    /// 把 `recalled` 写进 `state/models/catalog.json`：它已经不是在用的那份了（换过版本）的不写，不然会把新的盖成旧的。写不成
    /// 的记一行，内存里照样用：它是派生数据。
    fn save(&self, recalled: &Arc<Mutex<Recalled>>) {
        let _saving = lock(&self.recall.saving);
        let current = self
            .current_recalled()
            .is_some_and(|current| Arc::ptr_eq(&current, recalled));
        if !current {
            return;
        }
        let recalled = lock(recalled).clone();
        self.write_recall(&recalled);
    }

    /// 写 `state/models/catalog.json`；写不成 JSON 的（不会发生）记一行。
    fn write_recall(&self, recalled: &Recalled) {
        match recalled.to_json() {
            Ok(text) => self.write(RECALL_FILE, &text),
            Err(error) => {
                tracing::warn!(target: TARGET, file = RECALL_FILE, error = %error, "model data not written");
            }
        }
    }
}

/// 把配置 `resolved` 里写过的都问一遍：每一家配好的供应商（[`provider::provider`]），和它写过的每个模型的资料
/// （[`facts`]）。路由、回合开始、池的成员、配置检查问的都在里面。
fn ask_configured(resolved: &Resolved, knowledge: &Knowledge<'_>) {
    let values = resolved.values();
    let mut models: BTreeSet<(String, String)> = named(&values).into_iter().collect();
    if let Some(Ok(Reference::Model { provider, model })) = UseSettings::from(&values)
        .embedding
        .map(|text| Reference::parse(&text))
    {
        models.insert((provider, model));
    }
    let providers = provider::configured(&values);
    for id in &providers {
        models.extend(
            miyu_config::key::names(values.keys(), "providers.<id>.models.<model>", &[id])
                .into_iter()
                .map(|model| (id.clone(), model)),
        );
    }
    for id in &providers {
        let Ok(found) = provider::provider(&values, knowledge, id) else {
            continue;
        };
        for (_, model) in models.iter().filter(|(provider, _)| provider == id) {
            drop(facts(resolved, knowledge, &found, model));
        }
    }
}

/// 拿一把锁；别的线程拿着时出了 panic 的照样用：里面的每一样都是整条放进去的。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
