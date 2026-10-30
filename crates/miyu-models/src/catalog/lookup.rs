//! 查目录的两个切口（施工 V-25 上，`docs/blueprint/models.md`「怎么走」第二条第 4 条）：路由每一轮读目录，最后只落到这两处——
//! 认一家配好的供应商、取目录里那一家的几格（[`provider_side`]），对一个模型、取目录里那一条（[`model_side`]）。合资料
//! （[`crate::provider::provider`]、[`crate::facts::facts`]）只经 [`CatalogLookup`] 取这两样，不再直接碰目录。
//!
//! 为什么切在这里：整份目录八千多个模型、常驻约 3 MB，可一个核心真问到的只有配置里写的那几家、那几个模型。两个函数是纯的：
//! 同一份目录、同样的输入，答的一定一样。所以答过的可以照输入记下来（[`Recalled`]），以后照记下的答，和现算逐字一样——
//! 目录平时就不用常驻（施工 V-25 下）。这一步只切开、定接口，目录照旧常驻，合资料照旧用整份目录现算。

use std::sync::atomic::Ordering;
use std::sync::{MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use super::recall::{Recall, Recalled, Recalling};
use super::{Catalog, CatalogModel, CatalogSource, Loaded};
use crate::matching::{Found, Recognized, Vendors, find, recognize};

/// 一家配好的供应商在目录里的那一面（[`provider_side`]）：认成了哪一家、怎么认的，和那一家的几格。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSide {
    /// 认成了目录里的哪一家、怎么认的。
    pub recognized: Recognized,
    /// 那一家的 AI SDK 包名：照档案的 `[npm]` 换驱动（「怎么走」第一条第 2 条）。
    pub npm: Option<String>,
    /// 那一家的地址：手写的、档案的都没有时用它。
    pub api: Option<String>,
    /// 那一家的显示名（`model.list` 以后照它交，这一步还没人读）。
    pub name: Option<String>,
}

/// 一个模型在目录里的那一面（[`model_side`]）：对目录的结果，和对上的那一条。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSide {
    /// 四层对目录的结果（[`crate::matching::find`]）。
    pub found: Found,
    /// 对上时目录里那一条的原样拷贝；手写指定的不存在（`Missing`）、没对上（`Nothing`）的没有。
    pub entry: Option<CatalogModel>,
}

impl ModelSide {
    /// 没对上：手头没有目录时也是它（和以前「目录还没读完当没有」一样）。
    pub fn nothing() -> ModelSide {
        ModelSide {
            found: Found::Nothing,
            entry: None,
        }
    }
}

/// 认一家配好的供应商（编号 `id`、字面地址 `url`、手写的 `catalog` 即 `written`，照 [`crate::matching::recognize`]），
/// 认出来了交回那一家的包名、地址、显示名；认不出的没有。
///
/// 纯函数：同一份目录、同样的输入，答的一定一样（[`Recalled`] 照这一点记）。
pub fn provider_side(
    catalog: &Catalog,
    id: &str,
    url: Option<&str>,
    written: Option<&str>,
) -> Option<ProviderSide> {
    let recognized = recognize(catalog, id, url, written)?;
    // 认出来的一定是目录里有的那一家；万一没有（不该发生），几格都当没写，和以前 `listed` 是空的一样。
    let listed = catalog.provider(&recognized.provider);
    Some(ProviderSide {
        npm: listed.and_then(|listed| listed.npm.clone()),
        api: listed.and_then(|listed| listed.api.clone()),
        name: listed.and_then(|listed| listed.name.clone()),
        recognized,
    })
}

/// 照四层对一个模型（`recognized` 是这家在目录里是哪一家，`model` 是模型名，`written` 是模型手写的 `catalog`，照
/// [`crate::matching::find`]），对上了连同目录里那一条一起交回。
///
/// 纯函数：同一份目录、同一张认原厂的表 `vendors`、同样的输入，答的一定一样。
pub fn model_side(
    catalog: &Catalog,
    vendors: &Vendors,
    recognized: Option<&str>,
    model: &str,
    written: Option<&str>,
) -> ModelSide {
    let found = find(catalog, vendors, recognized, model, written);
    let entry = match &found {
        Found::Matched(matched) => catalog.model(&matched.provider, &matched.model).cloned(),
        Found::Missing(_) | Found::Nothing => None,
    };
    ModelSide { found, entry }
}

/// 合资料时怎么查目录（施工 V-25 上）：两种，答的逐字一样。
///
/// - [`CatalogLookup::Full`]：拿着整份目录现算（就是调 [`provider_side`]、[`model_side`]）。
/// - [`CatalogLookup::Recalled`]：照记下的答（[`Recalling`]），没记过的借整份目录现算、记下；没有整份目录可借的当没有目录。
///
/// 写成枚举不写成 trait：只有这两种、都在这个 crate 里，没有别人要接进来；枚举 `Copy`，[`crate::Knowledge`] 照旧整份
/// 拷着走，造的地方写 `catalog.map(CatalogLookup::Full)` 一句，不用另找地方放一个 `dyn` 的实现（这个 crate 里也没有别的
/// trait）。两种答的一样由 `recall/tests.rs` 逐个输入比着守。
#[derive(Debug, Clone, Copy)]
pub enum CatalogLookup<'a> {
    /// 整份目录现算。
    Full(&'a Loaded),
    /// 照记下的答。
    Recalled(Recalling<'a>),
}

impl CatalogLookup<'_> {
    /// 一家配好的供应商在目录里的那一面（[`provider_side`]）。照记下的答、又说不出来的（没记过、没有整份目录可借），当认
    /// 不出——和以前「目录还没读完」一样——并在 [`Recalling::missed`] 立一下，调的一方照它把这一遍作废重来。
    pub fn provider_side(
        &self,
        id: &str,
        url: Option<&str>,
        written: Option<&str>,
    ) -> Option<ProviderSide> {
        match self {
            CatalogLookup::Full(loaded) => provider_side(&loaded.catalog, id, url, written),
            CatalogLookup::Recalled(recalling) => {
                let answer = lock(recalling).provider_side(
                    recalling.catalog,
                    id,
                    url,
                    written,
                    recalling.remember,
                );
                match answer {
                    Recall::Known(side) => side,
                    Recall::Unknown => {
                        missed(recalling);
                        None
                    }
                }
            }
        }
    }

    /// 一个模型在目录里的那一面（[`model_side`]）。照记下的答、又说不出来的，当没对上，同样立一下。
    pub fn model_side(
        &self,
        vendors: &Vendors,
        recognized: Option<&str>,
        model: &str,
        written: Option<&str>,
    ) -> ModelSide {
        match self {
            CatalogLookup::Full(loaded) => {
                model_side(&loaded.catalog, vendors, recognized, model, written)
            }
            CatalogLookup::Recalled(recalling) => {
                let answer = lock(recalling).model_side(
                    recalling.catalog,
                    vendors,
                    recognized,
                    model,
                    written,
                    recalling.remember,
                );
                match answer {
                    Recall::Known(side) => side,
                    Recall::Unknown => {
                        missed(recalling);
                        ModelSide::nothing()
                    }
                }
            }
        }
    }

    /// 目录什么时候拉的：`meta` 里的原样（来源的 `fetched`，[`crate::facts::Source::catalog`]）。
    pub fn fetched(&self) -> String {
        match self {
            CatalogLookup::Full(loaded) => loaded.fetched.clone(),
            CatalogLookup::Recalled(recalling) => lock(recalling).fetched().to_string(),
        }
    }

    /// 哪一份目录。
    pub fn source(&self) -> CatalogSource {
        match self {
            CatalogLookup::Full(loaded) => loaded.source,
            CatalogLookup::Recalled(recalling) => lock(recalling).source(),
        }
    }
}

/// 说不出来：立一下，调的一方照它作废这一遍、借整份目录重来（[`Recalling::missed`]）。
fn missed(recalling: &Recalling<'_>) {
    if let Some(flag) = recalling.missed {
        flag.store(true, Ordering::Relaxed);
    }
}

/// 拿记下的那一份的锁。别的线程查到一半出了 panic 的，记下的照样能用：每一条都是查完才整条放进去的，不会只写了一半。
fn lock<'a>(recalling: &Recalling<'a>) -> MutexGuard<'a, Recalled> {
    recalling
        .recalled
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}
