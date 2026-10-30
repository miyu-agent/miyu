//! 查模型资料要的几份（`docs/blueprint/models.md`「怎么走」第一、二条，施工 8-7）：档案、认原厂的表、在用的目录、怎么查
//! 目录（施工 V-25 上）、用出来的、供应商的列表。核心一份，执行器拿着；查的时候借一份 [`Knowledge`] 进来，这一层不碰文件。

use std::collections::BTreeMap;

use crate::catalog::{CatalogLookup, Loaded};
use crate::matching::Vendors;
use crate::observed::{Learned, ProviderList};
use crate::profile::Profiles;

/// 这一刻手头的资料。
#[derive(Debug, Clone, Copy)]
pub struct Knowledge<'a> {
    /// 认得出的供应商的档案。
    pub profiles: &'a Profiles,
    /// 认原厂的表。
    pub vendors: &'a Vendors,
    /// 在用的整份目录；还没读完、都读不了的没有。合一家供应商、一个模型的资料不看它，看 [`Knowledge::lookup`]；要整份
    /// 目录的（`model.list` 的发布日期、第一次接入、搜目录、图标）看它。
    pub catalog: Option<&'a Loaded>,
    /// 合资料时怎么查目录（施工 V-25 上，[`CatalogLookup`]）：[`crate::provider::provider`]、[`crate::facts::facts`] 只经它认
    /// 供应商、对模型。没有的当没有目录。现在一律是整份目录现算（`catalog.map(CatalogLookup::Full)`）。
    pub lookup: Option<CatalogLookup<'a>>,
    /// 用出来的。
    pub learned: &'a Learned,
    /// 供应商的列表：配好的编号 → 列表。
    pub lists: &'a BTreeMap<String, ProviderList>,
}
