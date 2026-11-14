//! 主人在模型那边配的哪一家供应商是目录里的某一家、它的 key 是哪一把（施工 PK-1）：联网包的 DeepSeek 搜索没单独配 key 时照它
//! 借（设计 36 第二节第 1 条，原来在 `miyu-net` 的 `deepseek_provider`，施工 PK-1 挪到这里），批了 `provider.key` 的扩展照它拿
//! （端点的 `extensions/provider_key.rs`）。只看配置写了什么，不查档案、目录。
//!
//! 一家供应商 `providers.<id>` 算目录里的 `catalog` 那一家：
//!
//! 1. 写了 `catalog` 的，照它（写的是别家就不算，地址再像也不算）；
//! 2. 没写 `catalog`、写了字面地址的，地址里有那一家的官方地址（[`OFFICIAL`]）的算；
//! 3. 两样都没写的，编号就是 `catalog` 的算：核心照编号找档案（[`crate::provider::provider`]），第一次接入配 DeepSeek 写的
//!    就是这个样子（`providers.deepseek.key`，不写 `catalog`、不写地址）。
//!
//! 地址是环境变量的引用时查不出来，当没对上。几家都对上的照编号排第一的那一家；没写 key 的不算。

use miyu_config::secret::Reference;
use miyu_config::{Address, Values};

use crate::provider::configured;
use crate::settings::ProviderSettings;

/// 几家的官方地址：目录里的编号到地址里认的那一段。只放实测过的。
pub const OFFICIAL: [(&str, &str); 1] = [("deepseek", "api.deepseek.com")];

/// 配置 `values` 里算目录里 `catalog` 那一家、写了 key 的第一家供应商（照编号排）：编号和 key 的引用。没有的没有。
pub fn serving(values: &Values, catalog: &str) -> Option<(String, Reference)> {
    let official = OFFICIAL
        .iter()
        .find(|(id, _)| *id == catalog)
        .map(|(_, host)| *host);
    configured(values).into_iter().find_map(|id| {
        let settings = ProviderSettings::at(values, &[&id]);
        let key = settings.key?;
        let counts = match (&settings.catalog, &settings.base_url) {
            (Some(written), _) => written == catalog,
            (None, Some(Address::Literal(url))) => official.is_some_and(|host| url.contains(host)),
            (None, Some(Address::Env(_))) => false,
            (None, None) => id == catalog,
        };
        counts.then_some((id, key))
    })
}

#[cfg(test)]
mod tests;
