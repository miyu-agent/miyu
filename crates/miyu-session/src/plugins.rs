//! 场所插件（施工 O-60，`docs/blueprint/venues.md`「场所插件」）：扩展 `provide` 的工具可以带 `plugin`（[`miyu_tool::Spec::plugin`]），
//! 带了的是场所插件的工具。场所会话里，只有那个场所开着这个插件才给（通讯平台的桥照场所说开哪些，`venue.plugins`）；核心、
//! 桥刚重启、桥还没说过的场所一件都不给（2026-10-24 项目主人：不开就不该出现在工具集，白耗 token）。本机会话不看这个标记。
//! 核心在内存里记。会话两处照它：
//!
//! - 每个回合开始重筛工具面（`actor/persona.rs`）：没开的从发给模型的工具面拿掉、开了的补上，在跑的那一轮不动；
//! - 执行器派一次调用时（`tools.rs`）：这一刻没开的当目录里没有，回「暂时不可用」那一句，不跑。桥一说关就当场停（禁言、
//!   踢人这种），不等下一轮。
//!
//! 不带 `plugin` 的工具（普通的工具包）照旧，不归这里。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, PoisonError, RwLock};

use miyu_kernel::id::VenueId;
use miyu_policy::ToolEntry;
use miyu_tool::Catalog;

use crate::agents::LOCAL;

/// 每个场所开着的场所插件：一个核心一份，造会话、载入会话时交给会话。只在内存里。
#[derive(Debug, Clone, Default)]
pub struct VenuePlugins(Arc<RwLock<BTreeMap<VenueId, BTreeSet<String>>>>);

impl VenuePlugins {
    /// 场所 `venue` 开着的整份换成 `on`；空的拿掉（一件都不给）。
    pub fn set(&self, venue: VenueId, on: BTreeSet<String>) {
        let mut all = self.0.write().unwrap_or_else(PoisonError::into_inner);
        if on.is_empty() {
            all.remove(&venue);
        } else {
            all.insert(venue, on);
        }
    }

    /// 场所 `venue` 开着的；没说过的是空的。
    #[must_use]
    pub fn on(&self, venue: &VenueId) -> BTreeSet<String> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(venue)
            .cloned()
            .unwrap_or_default()
    }

    /// 场所 `venue` 的会话这一刻给不给 `name` 这一件（目录照 `catalog`）：不是场所插件的、本机会话的照给；场所插件的，那个
    /// 场所开着它才给。目录里没有的不归这里（照给，没有就是没有）。
    #[must_use]
    pub fn gives(&self, venue: &VenueId, catalog: &Catalog, name: &str) -> bool {
        self.allows(venue, plugin_of(catalog, name).as_deref())
    }

    /// 场所 `venue` 的会话这一刻给不给属于场所插件 `plugin` 的工具：不是场所插件的（`None`）、本机会话的照给。
    #[must_use]
    pub fn allows(&self, venue: &VenueId, plugin: Option<&str>) -> bool {
        let Some(plugin) = plugin else {
            return true;
        };
        venue.as_str() == LOCAL
            || self
                .0
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .get(venue)
                .is_some_and(|on| on.contains(plugin))
    }
}

/// `name` 这一件是哪个场所插件的；不是的、目录里没有的没有。
pub(crate) fn plugin_of(catalog: &Catalog, name: &str) -> Option<String> {
    catalog.get(name)?.spec().plugin.clone()
}

/// 照场所 `venue` 这一刻开着的，从工具面 `face` 拿掉不给的场所插件工具（目录照 `catalog`）；交回留下的场所插件工具的名字，
/// 照名字排（快照的 `plugin_tools`）。
pub(crate) fn sift(
    face: &mut Vec<ToolEntry>,
    catalog: &Catalog,
    (plugins, venue): (&VenuePlugins, &VenueId),
) -> Vec<String> {
    face.retain(|entry| plugins.gives(venue, catalog, &entry.name));
    let mut given: Vec<String> = face
        .iter()
        .filter(|entry| plugin_of(catalog, &entry.name).is_some())
        .map(|entry| entry.name.clone())
        .collect();
    given.sort();
    given
}

/// 工具面 `face` 上的场所插件工具的名字，照名字排（目录照 `catalog`）。
pub(crate) fn present(face: &[ToolEntry], catalog: &Catalog) -> Vec<String> {
    let mut names: Vec<String> = face
        .iter()
        .filter(|entry| plugin_of(catalog, &entry.name).is_some())
        .map(|entry| entry.name.clone())
        .collect();
    names.sort();
    names
}

/// 回合开始重筛（`actor/persona.rs`）：`face` 拿掉全部场所插件工具，补上 `fresh`（照这一刻的目录、预设筛出来的）里这个
/// 场所开着的。别的工具原样。
pub(crate) fn replug(
    face: Vec<ToolEntry>,
    fresh: &[ToolEntry],
    catalog: &Catalog,
    (plugins, venue): (&VenuePlugins, &VenueId),
) -> Vec<ToolEntry> {
    let plugged = |entry: &ToolEntry| plugin_of(catalog, &entry.name).is_some();
    face.into_iter()
        .filter(|entry| !plugged(entry))
        .chain(
            fresh
                .iter()
                .filter(|entry| plugged(entry) && plugins.gives(venue, catalog, &entry.name))
                .cloned(),
        )
        .collect()
}

#[cfg(test)]
mod tests;
