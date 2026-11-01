//! 记下来的查目录的答（施工 V-25 上，`docs/blueprint/models.md`「怎么走」第二条第 4 条）：[`provider_side`]、
//! [`model_side`] 是纯函数，同一份目录、同样的输入答的一定一样，所以照输入把答记进两张表，以后照表答，和整份目录现算逐字
//! 一样。负的答（认不出、没对上、手写指定的不存在）也记：路由每一轮都要问，没记的就得再借整份目录。
//!
//! - 记的时候认这一份是哪一份目录：`of`（版本串，由调的一方定，例如目录的指纹）、哪一份（[`CatalogSource`]）、什么时候
//!   拉的（`fetched`，来源照它写）。换了目录的，调的一方照 `of` 认出来、换一份新的，这里不管。
//! - 查不到的：借进来的整份目录现算、记下、交回；没有整份目录可借的交回 [`Recall::Unknown`]，由调的一方决定怎么办（合资料
//!   时当没有目录，[`crate::catalog::CatalogLookup`]）。
//! - 记不记由调的一方说（`remember`）：配置里写过、真问过的才该记；临时问一句的（搜目录、第一次接入试一家）不记，免得表
//!   越记越大。
//! - 写成 JSON、从 JSON 读回（[`Recalled::to_json`]、[`Recalled::from_json`]），照 `digest.rs` 的路子带写法的版本；读不懂的
//!   报错，由调的一方处理。存盘、接进路由在 `miyu-session` 的 `ModelData`（施工 V-25 下）。
//! - 换了目录的：照新目录把记下的输入全部重算（[`Recalled::rebuilt`]），调的一方原子地换上新的一份。
//! - 第三张表（施工 V-32）：目录里一家的模型清单（[`listing_side`]），`model.list` 列整份时问；同一个类型另存一份
//!   `state/models/listing.json`，平时不进内存（`miyu-session` 的 `route/shared/listing.rs`），两份的答照 [`Recalled::merge`]
//!   并在一起查。路由那份（`catalog.json`）里这张表一直是空的，写出来的字和以前一个字节不差。

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use super::lookup::{
    CatalogListing, ModelSide, ProviderSide, listing_side, model_side, provider_side,
};
use super::{CatalogSource, Loaded};
use crate::matching::Vendors;

/// 写法的版本：这一份的样子变了就加一。
pub const RECALL_VERSION: u32 = 1;

/// 照记下的答：说得出来的，和说不出来的（没记过，又没有整份目录可借）。
#[derive(Debug, Clone, PartialEq)]
pub enum Recall<T> {
    /// 说得出来：记下的，或者借整份目录现算的。
    Known(T),
    /// 说不出来：没记过这个输入，手头也没有整份目录。
    Unknown,
}

impl<T> Recall<T> {
    /// 说得出来的那个答；说不出来的没有。
    pub fn known(self) -> Option<T> {
        match self {
            Recall::Known(answer) => Some(answer),
            Recall::Unknown => None,
        }
    }
}

/// 问 [`provider_side`] 的输入：编号、字面地址、手写的 `catalog`。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct ProviderAsk {
    id: String,
    url: Option<String>,
    written: Option<String>,
}

/// 问 [`model_side`] 的输入：这家在目录里是哪一家、模型名、模型手写的 `catalog`。认原厂的表不算进来：它是出厂的资料，
/// 变了由调的一方算进 `of`。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct ModelAsk {
    recognized: Option<String>,
    model: String,
    written: Option<String>,
}

/// 记下来的查目录的答：一份目录（`of`）的两张表。
#[derive(Debug, Clone, PartialEq)]
pub struct Recalled {
    of: String,
    source: CatalogSource,
    fetched: String,
    providers: BTreeMap<ProviderAsk, Option<ProviderSide>>,
    models: BTreeMap<ModelAsk, ModelSide>,
    /// 目录里一家的模型清单（施工 V-32）：目录里的编号 → 清单，目录里没有这一家的也记（`None`）。
    listings: BTreeMap<String, Option<CatalogListing>>,
}

/// 写成 JSON、从 JSON 读回的样子：表的键不是字，写成一排「输入，答」。
#[derive(Serialize, Deserialize)]
struct Stored {
    v: u32,
    of: String,
    source: CatalogSource,
    fetched: String,
    providers: Vec<(ProviderAsk, Option<ProviderSide>)>,
    models: Vec<(ModelAsk, ModelSide)>,
    /// 施工 V-32：空的不写，路由那份 `catalog.json` 的字照旧；没写的读成空的，以前写的照样读得回来。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    listings: Vec<(String, Option<CatalogListing>)>,
}

impl Recalled {
    /// 空的一份：记的是版本串 `of`、哪一份 `source`、`fetched` 拉的那一份目录的答。
    pub fn new(of: &str, source: CatalogSource, fetched: &str) -> Recalled {
        Recalled {
            of: of.to_string(),
            source,
            fetched: fetched.to_string(),
            providers: BTreeMap::new(),
            models: BTreeMap::new(),
            listings: BTreeMap::new(),
        }
    }

    /// 版本串：记的是哪一份目录，调的一方照它认这一份还能不能用。
    pub fn of(&self) -> &str {
        &self.of
    }

    /// 哪一份目录。
    pub fn source(&self) -> CatalogSource {
        self.source
    }

    /// 目录什么时候拉的：`meta` 里的原样。
    pub fn fetched(&self) -> &str {
        &self.fetched
    }

    /// 记了几条：供应商的、模型的。
    pub fn len(&self) -> (usize, usize) {
        (self.providers.len(), self.models.len())
    }

    /// 三张表一共记了几条（施工 V-32：看这一次多记了没有）。
    pub fn entries(&self) -> usize {
        self.providers.len() + self.models.len() + self.listings.len()
    }

    /// 把 `other` 记下的并进来（施工 V-32：`model.list` 把路由那份和 `listing.json` 并在一起查）：这里没有的输入照 `other` 的
    /// 答记上，有的不动。两份必须是同一份目录（`of` 一样）的答，这里不查，由调的一方比。
    pub fn merge(&mut self, other: Recalled) {
        for (ask, side) in other.providers {
            self.providers.entry(ask).or_insert(side);
        }
        for (ask, side) in other.models {
            self.models.entry(ask).or_insert(side);
        }
        for (provider, listing) in other.listings {
            self.listings.entry(provider).or_insert(listing);
        }
    }

    /// 照记下的答 [`provider_side`]：记过的照记下的；没记过的借 `catalog` 现算，`remember` 为真时记下；没有 `catalog` 的
    /// 说不出来。`catalog` 必须是记下时的那一份（`of`），这里不查。
    pub fn provider_side(
        &mut self,
        catalog: Option<&Loaded>,
        id: &str,
        url: Option<&str>,
        written: Option<&str>,
        remember: bool,
    ) -> Recall<Option<ProviderSide>> {
        let ask = ProviderAsk {
            id: id.to_string(),
            url: url.map(str::to_string),
            written: written.map(str::to_string),
        };
        if let Some(side) = self.providers.get(&ask) {
            return Recall::Known(side.clone());
        }
        let Some(loaded) = catalog else {
            return Recall::Unknown;
        };
        let side = provider_side(&loaded.catalog, id, url, written);
        if remember {
            self.providers.insert(ask, side.clone());
        }
        Recall::Known(side)
    }

    /// 照记下的答 [`model_side`]：同 [`Recalled::provider_side`]。`vendors` 只在借 `catalog` 现算时用。
    pub fn model_side(
        &mut self,
        catalog: Option<&Loaded>,
        vendors: &Vendors,
        recognized: Option<&str>,
        model: &str,
        written: Option<&str>,
        remember: bool,
    ) -> Recall<ModelSide> {
        let ask = ModelAsk {
            recognized: recognized.map(str::to_string),
            model: model.to_string(),
            written: written.map(str::to_string),
        };
        if let Some(side) = self.models.get(&ask) {
            return Recall::Known(side.clone());
        }
        let Some(loaded) = catalog else {
            return Recall::Unknown;
        };
        let side = model_side(&loaded.catalog, vendors, recognized, model, written);
        if remember {
            self.models.insert(ask, side.clone());
        }
        Recall::Known(side)
    }

    /// 照记下的答 [`listing_side`]（施工 V-32）：同 [`Recalled::provider_side`]。
    pub fn listing(
        &mut self,
        catalog: Option<&Loaded>,
        provider: &str,
        remember: bool,
    ) -> Recall<Option<CatalogListing>> {
        if let Some(listing) = self.listings.get(provider) {
            return Recall::Known(listing.clone());
        }
        let Some(loaded) = catalog else {
            return Recall::Unknown;
        };
        let listing = listing_side(&loaded.catalog, provider);
        if remember {
            self.listings.insert(provider.to_string(), listing.clone());
        }
        Recall::Known(listing)
    }

    /// 换了一份目录（施工 V-25 下：后台拉到了新的、缓存被别的核心换过）：照新的 `loaded`、认原厂的表 `vendors` 把记下的每
    /// 个输入重算一遍，交回版本串是 `of` 的新一份。旧的不动：还在用它查的照旧查完，查完的人换成新的（调的一方原子地换）。
    ///
    /// 重算的只是记下的这几十条输入，不是整份目录：认出的那一家变了的，模型那一面以后会拿新的「认出的那一家」来问，那时
    /// 没记过、再借一次整份目录。旧的键留着不删：只多不少，输入都是配置里写过、真问过的。
    pub fn rebuilt(&self, loaded: &Loaded, vendors: &Vendors, of: &str) -> Recalled {
        let providers = self
            .providers
            .keys()
            .map(|ask| {
                let side = provider_side(
                    &loaded.catalog,
                    &ask.id,
                    ask.url.as_deref(),
                    ask.written.as_deref(),
                );
                (ask.clone(), side)
            })
            .collect();
        let models = self
            .models
            .keys()
            .map(|ask| {
                let side = model_side(
                    &loaded.catalog,
                    vendors,
                    ask.recognized.as_deref(),
                    &ask.model,
                    ask.written.as_deref(),
                );
                (ask.clone(), side)
            })
            .collect();
        let listings = self
            .listings
            .keys()
            .map(|provider| (provider.clone(), listing_side(&loaded.catalog, provider)))
            .collect();
        Recalled {
            of: of.to_string(),
            source: loaded.source,
            fetched: loaded.fetched.clone(),
            providers,
            models,
            listings,
        }
    }

    /// 记下的每一份清单里的每个模型，照没手写 `catalog` 的问法（认出的是清单那一家）借 `loaded` 问一遍、记下（施工 V-32）：
    /// 换了目录、清单里多了模型的，`model.list` 头一回列它时不用再读整份目录。`loaded` 必须是记下时的那一份（`of`）。
    pub fn fill_listed(&mut self, loaded: &Loaded, vendors: &Vendors) {
        let asks: Vec<ModelAsk> = self
            .listings
            .iter()
            .filter_map(|(provider, listing)| Some((provider, listing.as_ref()?)))
            .flat_map(|(provider, listing)| {
                listing.models.keys().map(|model| ModelAsk {
                    recognized: Some(provider.clone()),
                    model: model.clone(),
                    written: None,
                })
            })
            .collect();
        for ask in asks {
            if let std::collections::btree_map::Entry::Vacant(vacant) = self.models.entry(ask) {
                let ask = vacant.key();
                let side = model_side(
                    &loaded.catalog,
                    vendors,
                    ask.recognized.as_deref(),
                    &ask.model,
                    None,
                );
                vacant.insert(side);
            }
        }
    }

    /// 写成 JSON：带写法的版本 [`RECALL_VERSION`]。
    ///
    /// # Errors
    ///
    /// 写不成 JSON（不该发生：键都写成了一排，数都是目录里读出来的）：原话说是这一份。
    pub fn to_json(&self) -> Result<String, String> {
        let stored = Stored {
            v: RECALL_VERSION,
            of: self.of.clone(),
            source: self.source,
            fetched: self.fetched.clone(),
            providers: self
                .providers
                .iter()
                .map(|(ask, side)| (ask.clone(), side.clone()))
                .collect(),
            models: self
                .models
                .iter()
                .map(|(ask, side)| (ask.clone(), side.clone()))
                .collect(),
            listings: self
                .listings
                .iter()
                .map(|(provider, listing)| (provider.clone(), listing.clone()))
                .collect(),
        };
        serde_json::to_string(&stored)
            .map_err(|error| format!("catalog recall not writable: {error}"))
    }

    /// 从 [`Recalled::to_json`] 写的 JSON 读回。认不认这一份（`of` 对不对得上）由调的一方照 [`Recalled::of`] 定。
    ///
    /// # Errors
    ///
    /// 读不懂、写法的版本不是 [`RECALL_VERSION`]：原话说是这一份，和为什么。
    pub fn from_json(text: &str) -> Result<Recalled, String> {
        let stored: Stored = serde_json::from_str(text)
            .map_err(|error| format!("catalog recall not readable: {error}"))?;
        if stored.v != RECALL_VERSION {
            return Err(format!(
                "catalog recall not readable: version {} is not {RECALL_VERSION}",
                stored.v
            ));
        }
        Ok(Recalled {
            of: stored.of,
            source: stored.source,
            fetched: stored.fetched,
            providers: stored.providers.into_iter().collect(),
            models: stored.models.into_iter().collect(),
            listings: stored.listings.into_iter().collect(),
        })
    }
}

/// 合资料时照记下的答（[`crate::catalog::CatalogLookup::Recalled`]）：记下的那一份（锁着，查的时候可能要记）、能借的整份目录、
/// 记不记，说不出来时记在哪。
#[derive(Debug, Clone, Copy)]
pub struct Recalling<'a> {
    /// 记下的那一份。
    pub recalled: &'a Mutex<Recalled>,
    /// 没记过的借它现算；没有的说不出来（当没有目录）。必须是记下时的那一份（`of`）。
    pub catalog: Option<&'a Loaded>,
    /// 没记过、借整份目录现算的记不记：配置里写过、真问过的记，临时问一句的不记。
    pub remember: bool,
    /// 说不出来（[`Recall::Unknown`]）时在这里立一下（施工 V-25 下）：这一次合出来的当没有目录，不能交出去——调的一方
    /// 照它把整份目录读回来、重合一遍（`miyu-session` 的 `ModelData::with`）。供应商那一面说不出来时，合资料会拿「认不出」
    /// 再去问模型那一面，交回的是认不出时的答，和有目录时不一样，所以只要立过一次，这一遍合出来的整个作废。没有的不立。
    pub missed: Option<&'a AtomicBool>,
}

#[cfg(test)]
mod tests;
