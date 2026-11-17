//! 中转站的价格照原厂借（施工 PR-1，`docs/blueprint/models.md`「怎么走」第二条第 6 条第 3 款，2026-11-17 项目主人定「借原厂价、
//! 不标」）：第 3、4 层几家同名、里面既没有第 2 层认出的那一家也没有原厂的，条目照编号的字节序取第一家，价格不照它（中转的价
//! 不是官方价），去原厂那一家找同一个模型借。
//!
//! 原厂照 [`Vendors`] 认（同第 2 款）；照表里的先后一家家找，先找到的算。在一家里照这个先后挑，先对上的算：
//!
//! 1. 规整以后的名字是条目的规整名 N，或者 N 在 `-` 处断开的最长的前缀（只有一段、没有数字的通用名只在正好相等时算，同第 4 层）：
//!    `deepseek-v4-1-flash-261017` 借 `deepseek-v4.1-flash`。
//! 2. `family` 规整以后和条目的一样的：和 N 从头数相同的段最多的那个，一样多的取名字照字节排第一的。原厂没列这个版本时靠它：
//!    `deepseek-v4-1-flash-261017`（家族 `deepseek-flash`）借 `deepseek-v4-flash`。
//!
//! 只挑有价格的。都没有的不借，这个模型没有价格。

use crate::catalog::{Catalog, CatalogModel, Entry};

use super::{Vendors, normalize, prefixes};

/// 价格借哪一条：目录里的编号和模型名。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Priced {
    /// 目录里的编号。
    pub provider: String,
    /// 目录里的模型名。
    pub model: String,
}

impl Priced {
    /// 目录里的 `provider` 家的 `model`。
    pub(super) fn of(provider: &str, model: &str) -> Priced {
        Priced {
            provider: provider.to_string(),
            model: model.to_string(),
        }
    }
}

/// 几家同名的 `entries`（照字节排、不是空的）里没有原厂的：照第一条的规整名去原厂那一家借价格。借不到的没有。
pub(super) fn from_vendor(
    catalog: &Catalog,
    vendors: &Vendors,
    entries: &[Entry],
) -> Option<Priced> {
    let (provider, model) = &entries[0];
    let family = catalog
        .model(provider, model)
        .and_then(|entry| entry.family.as_deref());
    let normalized = normalize(model);
    let ids = entries
        .iter()
        .map(|(provider, model)| {
            let family = catalog
                .model(provider, model)
                .and_then(|entry| entry.family.as_deref());
            vendors.of(family, model)
        })
        .find(|ids| !ids.is_empty())
        .unwrap_or_default();
    ids.iter().find_map(|vendor| {
        let listed = catalog.provider(vendor)?;
        let priced: Vec<(&str, &CatalogModel)> = listed
            .models
            .iter()
            .filter(|(_, entry)| entry.price.is_some())
            .map(|(name, entry)| (&**name, entry))
            .collect();
        let name =
            by_prefix(&priced, &normalized).or_else(|| by_family(&priced, family?, &normalized))?;
        Some(Priced::of(vendor, name))
    })
}

/// 第 1 条：规整名是 N 或者它最长的前缀的。
fn by_prefix<'a>(priced: &[(&'a str, &CatalogModel)], normalized: &str) -> Option<&'a str> {
    prefixes(normalized)
        .filter(|prefix| *prefix == normalized || !super::generic(prefix))
        .find_map(|prefix| {
            priced
                .iter()
                .find(|(name, _)| normalize(name) == prefix)
                .map(|(name, _)| *name)
        })
}

/// 第 2 条：家族一样的里和 N 从头数相同的段最多的，一样多的取字节序在前的（`priced` 照名字排）。
fn by_family<'a>(
    priced: &[(&'a str, &CatalogModel)],
    family: &str,
    normalized: &str,
) -> Option<&'a str> {
    let family = normalize(family);
    let mut best: Option<(usize, &str)> = None;
    for (name, entry) in priced {
        if entry.family.as_deref().map(normalize).as_deref() != Some(family.as_str()) {
            continue;
        }
        let shared = normalize(name)
            .split('-')
            .zip(normalized.split('-'))
            .take_while(|(a, b)| a == b)
            .count();
        if best.is_none_or(|(most, _)| shared > most) {
            best = Some((shared, name));
        }
    }
    best.map(|(_, name)| name)
}

#[cfg(test)]
mod tests;
