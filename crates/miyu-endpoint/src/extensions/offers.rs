//! 扩展交预设功能（施工 IM-6，接入通讯平台设计 06 第 4.6 节第 4 样）：`feature.offer {"features": [...]}`，交这个扩展
//! 现在有哪些功能，整份替换，空的是全撤。核心已有给人改全局开关的 `feature.set`（施工 X-27），所以这个叫 `offer`。
//!
//! - 照包记在内存里，不存盘：扩展每次连上、开关变了都交一遍，核心重启以后再交。进程退了（包括 retire）不清，
//!   免得她在终端里用不到这些工具、又没东西把扩展拉起来。
//! - 每一个 `{"id", "name", "summary"?, "tools"}`：`id` 写法同包的编号，不能和装了的功能、别的包交的撞；`name`、`summary`
//!   是一句字或者语言到字的表（同清单 `[features]`，一句字的当英文，照语言挑时谁都拿得到）；`tools` 必须是这个包登记过的工具。
//!   不对的回 `bad_params`（`field` 是 `features`），什么都不换。
//! - 交了的进预设菜单（`preset.get`），归这个包；列出来的工具归它，不归包自己的功能（[`miyu_policy::features::Features::of_tool`]）。
//!   开不开照预设：出厂「全部功能」`unlisted = "on"` 开，别的出厂预设关。开着的会话下一个回合照新的算（`PresetPlaces::offers`）。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, PoisonError};

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::phrases::Phrases;
use miyu_policy::features::Feature;

use super::TARGET;
use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// 一个交了的功能：给预设和工具面的那一份，加上给人看的名字、说明。
#[derive(Debug, Clone)]
pub(crate) struct Offer {
    pub(crate) feature: Feature,
    pub(crate) name: Phrases,
    pub(crate) summary: Phrases,
}

/// 交了的功能：包到那一整份。
#[derive(Default)]
pub(crate) struct Offers(Mutex<BTreeMap<String, Vec<Offer>>>);

impl Offers {
    fn table(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Vec<Offer>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 交了的全部，照包的编号、交的先后。
    pub(crate) fn of(&self) -> Vec<Offer> {
        self.table()
            .values()
            .flat_map(|offers| offers.iter().cloned())
            .collect()
    }

    /// 交了的功能（给预设、工具面的那一份）。
    pub(crate) fn features_of(&self) -> Vec<Feature> {
        self.of().into_iter().map(|offer| offer.feature).collect()
    }
}

/// `feature.offer` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfferParams {
    features: Vec<Offered>,
}

/// 交的一个功能。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Offered {
    id: String,
    name: Words,
    #[serde(default)]
    summary: Option<Words>,
    tools: Vec<String>,
}

/// 一句字，或者语言到字的表。
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Words {
    One(String),
    Many(Phrases),
}

impl Words {
    /// 照清单的写法存：一句字的当英文。
    fn phrases(self) -> Phrases {
        match self {
            Words::One(text) => Phrases::from([("en".to_string(), text)]),
            Words::Many(phrases) => phrases,
        }
    }
}

/// `feature.offer`：见模块说明，回 `{}`。不是核心拉起的扩展的连接 `not_an_extension`。
pub(crate) fn offer(core: &Core, caller: &Caller, params: OfferParams) -> Result<Value, Refusal> {
    let package = caller.package.as_deref().ok_or(Refusal::NOT_AN_EXTENSION)?;
    let offers = checked(core, package, params.features)?;
    let count = offers.len();
    let key = package.to_string();
    let mut table = core.extensions.offers.table();
    if offers.is_empty() {
        table.remove(&key);
    } else {
        table.insert(key, offers);
    }
    drop(table);
    tracing::info!(target: TARGET, package, features = count, "features offered");
    Ok(json!({}))
}

/// 查交的这一份：写法、撞不撞、工具是不是自己的。
fn checked(core: &Core, package: &str, offered: Vec<Offered>) -> Result<Vec<Offer>, Refusal> {
    let bad = || Refusal::bad_field("features");
    // 这个人现在的功能里，除了这个包上一次交的那些，都是占着的编号。
    let mine: BTreeSet<String> = core
        .extensions
        .offers
        .table()
        .get(package)
        .into_iter()
        .flatten()
        .map(|offer| offer.feature.id.clone())
        .collect();
    let installed = crate::scripts::features(core);
    let taken = |id: &str| installed.iter().any(|feature| feature.id == id) && !mine.contains(id);
    let catalog = core.tools.current();
    let own = |tool: &str| catalog.package_of(tool) == Some(package);
    let mut seen = BTreeSet::new();
    let mut offers = Vec::with_capacity(offered.len());
    for one in offered {
        if !miyu_config::secret::valid_name(&one.id)
            || taken(&one.id)
            || !seen.insert(one.id.clone())
            || !one.tools.iter().all(|tool| own(tool))
        {
            return Err(bad());
        }
        offers.push(Offer {
            feature: Feature {
                id: one.id,
                package: package.to_string(),
                tools: one.tools,
                service: None,
                script: None,
            },
            name: one.name.phrases(),
            summary: one.summary.map(Words::phrases).unwrap_or_default(),
        });
    }
    Ok(offers)
}
