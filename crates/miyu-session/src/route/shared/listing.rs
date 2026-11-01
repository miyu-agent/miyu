//! `model.list` 不读整份目录（施工 V-32，`docs/blueprint/models.md`「怎么走」第二条第 2 条第 7 款）：列整份要目录里那一家的
//! 全部模型（清单、每个模型对目录的答），第一次照整份目录算的时候记进 `state/models/listing.json`，以后每次 `model.list` 读这个
//! 小文件、和路由那份（`catalog.json`，常驻）并在一起照记下的答，用完就放——平时不在内存里。
//!
//! - 同一个类型（`miyu_models::catalog::Recalled`）、同一个版本串：`of` 和路由那份对不上的当没有；读不了、读不懂的记一行
//!   `WARN catalog listing unreadable`，当没有。
//! - 说不出来的（没记过：新配的供应商、列表新拉到的模型）照 `recall.rs` 的路子：整遍作废，拿补算的锁再读一次文件问一遍，还
//!   说不出来的临时读整份目录（[`ModelData::hold`]）、一家一家重合、记下、写盘。合资料照旧在 `observed` 的锁里，读目录、读写
//!   这个文件都不在里面。
//! - 有上限（[`LISTING_CAP`]）：记下一家以后模型那一面超过上限的，这一家这一次不记，答照样交（借整份目录现算的）。
//! - 换了目录（后台拉到新的、临时读回来的版本不一样、起来时版本对不上读了整份目录）：照新目录把文件里记下的键全部重算、清单
//!   里新多出来的模型顺手问好，写回（[`ModelData::relist`]），和路由那份一起换。
//! - 不清：配置里删掉的供应商记下的留着（只多不少，键带着认出的那一家和手写的 `catalog`，不会答错），只占盘，换版本时跟着
//!   重算。

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use miyu_models::Knowledge;
use miyu_models::catalog::{CatalogLookup, CatalogSource, Recalled, Recalling};

use crate::TARGET;

use super::ModelData;
use super::recall::{Held, lock};

/// `model.list` 记下的答存在 `state/models/` 下的这个文件里（施工 V-32）。
pub const LISTING_FILE: &str = "listing.json";

/// `listing.json` 里模型那一面最多记几条（施工 V-32，2026-10-31 定）：一家记下以后超了的，这一家这一次不记，`model.list`
/// 照旧临时读整份目录。一千条装得下两家最大的聚合商（nano-gpt 600、openrouter 387），文件约 0.5～0.7 MB。
pub const LISTING_CAP: usize = 1000;

/// `model.list` 照记下的答合出来的（[`ModelData::listing`]）：每一家一个答，照问的先后；在用的目录是哪一份、什么时候拉的
/// （`model.list` 的 `catalog`），还没有目录、都读不了的没有。
#[derive(Debug)]
pub struct Listed<R> {
    /// 每一家的答。
    pub answers: Vec<R>,
    /// 在用的目录：哪一份、`fetched`。
    pub catalog: Option<(CatalogSource, String)>,
}

impl ModelData {
    /// `model.list` 列整份（施工 V-32）：`ids` 每一家做一次 `work`，资料照路由那份并上 `listing.json` 记下的答；说不出来的
    /// 临时读整份目录补、记下、写盘。常驻的照整份目录现算。`work` 可能做两遍、作废的那一遍看到的是没有目录时的答（同
    /// [`ModelData::with`]）：只放查资料的纯活。慢：要读盘，在阻塞线程里调。
    pub fn listing<R>(
        &self,
        ids: &[String],
        mut work: impl FnMut(&Knowledge<'_>, &str) -> R,
    ) -> Listed<R> {
        let held = self.catalog.borrow().clone();
        let recalled = match held {
            None => {
                return Listed {
                    answers: ids
                        .iter()
                        .map(|id| self.run(None, None, |k| work(k, id)))
                        .collect(),
                    catalog: None,
                };
            }
            Some(Held::Full(loaded)) => {
                let catalog = loaded.as_deref();
                let lookup = catalog.map(CatalogLookup::Full);
                return Listed {
                    answers: ids
                        .iter()
                        .map(|id| self.run(catalog, lookup, |k| work(k, id)))
                        .collect(),
                    catalog: catalog.map(|loaded| (loaded.source, loaded.fetched.clone())),
                };
            }
            Some(Held::Recalled(recalled)) => recalled,
        };
        if let Some(listed) = self.listed_only(&recalled, ids, &mut work) {
            return listed;
        }
        // 同 `asking`：拿着补算的锁再问一遍（等锁时别人可能刚记下、写了盘），还说不出来的才读整份目录。
        let _missing = lock(&self.recall.missing);
        let recalled = self.current_recalled().unwrap_or(recalled);
        if let Some(listed) = self.listed_only(&recalled, ids, &mut work) {
            return listed;
        }
        let started = Instant::now();
        let holding = self.hold();
        // 读回来的版本不一样的已经换过一份（`hold`），照换过的那份并。
        let recalled = holding.recalled.clone().unwrap_or(recalled);
        let catalog = holding.loaded.as_deref();
        let mut base = self.listing_base(&recalled);
        let before = base.entries();
        let answers = ids
            .iter()
            .map(|id| {
                let trial = Mutex::new(base.clone());
                let lookup = CatalogLookup::Recalled(Recalling {
                    recalled: &trial,
                    catalog,
                    remember: true,
                    missed: None,
                });
                let answer = self.run(catalog, Some(lookup), |k| work(k, id));
                let trial = trial.into_inner().unwrap_or_else(PoisonError::into_inner);
                let models = trial.len().1;
                match models <= LISTING_CAP {
                    true => base = trial,
                    false => {
                        tracing::debug!(target: TARGET, models, cap = LISTING_CAP, "catalog listing over cap");
                    }
                }
                answer
            })
            .collect();
        tracing::debug!(target: TARGET, ms = started.elapsed().as_millis(), "catalog read for a listing not recalled");
        if base.entries() != before {
            self.save_listing(&recalled, &base);
        }
        Listed {
            answers,
            catalog: Some((base.source(), base.fetched().to_string())),
        }
    }

    /// 换了目录（施工 V-32）：照新目录 `loaded`（版本串 `of`）把 `listing.json` 里记下的键全部重算、写回。没有的、读不了的不管
    /// （下次 `model.list` 照新的记）。起来时版本对不上读了整份目录的（核心的 `begin`）、后台拉到新的、临时读回来的版本不一样
    /// 的（`recall.rs` 的 `swap`）都调它。
    pub fn relist(&self, loaded: &miyu_models::catalog::Loaded, of: &str) {
        let _saving = lock(&self.recall.saving);
        let Some(listing) = self.read_listing() else {
            return;
        };
        let mut rebuilt = listing.rebuilt(loaded, &self.vendors, of);
        // 新目录里清单多了的模型顺手问好；多到过了上限的不问，`model.list` 到时照旧读整份目录。
        let mut filled = rebuilt.clone();
        filled.fill_listed(loaded, &self.vendors);
        if filled.len().1 <= LISTING_CAP {
            rebuilt = filled;
        }
        self.write_recalled(LISTING_FILE, &rebuilt);
    }

    /// 只照记下的合一遍（路由那份并上 `listing.json`）：有说不出来的没有（这一遍作废）。
    fn listed_only<R>(
        &self,
        recalled: &Mutex<Recalled>,
        ids: &[String],
        work: &mut impl FnMut(&Knowledge<'_>, &str) -> R,
    ) -> Option<Listed<R>> {
        let base = Mutex::new(self.listing_base(recalled));
        let missed = std::sync::atomic::AtomicBool::new(false);
        let lookup = CatalogLookup::Recalled(Recalling {
            recalled: &base,
            catalog: None,
            remember: false,
            missed: Some(&missed),
        });
        let mut answers = Vec::with_capacity(ids.len());
        for id in ids {
            answers.push(self.run(None, Some(lookup), |k| work(k, id)));
            if missed.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
        }
        let base = lock(&base);
        Some(Listed {
            answers,
            catalog: Some((base.source(), base.fetched().to_string())),
        })
    }

    /// 路由那份 `recalled` 拷一份，并上 `listing.json` 记下的（版本串对得上的）。
    fn listing_base(&self, recalled: &Mutex<Recalled>) -> Recalled {
        let mut base = lock(recalled).clone();
        if let Some(listing) = self.read_listing()
            && listing.of() == base.of()
        {
            base.merge(listing);
        }
        base
    }

    /// 读 `listing.json`：没有的、没有 `state` 的没有；读不了、读不懂的记一行 `WARN`，当没有。
    fn read_listing(&self) -> Option<Recalled> {
        let path = self.dir.as_ref()?.join(LISTING_FILE);
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "catalog listing unreadable");
                return None;
            }
        };
        Recalled::from_json(&text)
            .map_err(|error| tracing::warn!(target: TARGET, error = %error, "catalog listing unreadable"))
            .ok()
    }

    /// 写 `listing.json`：路由那份已经换过版本的（`recalled` 不是在用的那份了）不写，不然会把重算过的新一份盖成旧的。
    fn save_listing(&self, recalled: &Arc<Mutex<Recalled>>, listing: &Recalled) {
        let _saving = lock(&self.recall.saving);
        let current = self
            .current_recalled()
            .is_some_and(|current| Arc::ptr_eq(&current, recalled));
        if current {
            self.write_recalled(LISTING_FILE, listing);
        }
    }
}
