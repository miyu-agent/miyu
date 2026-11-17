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
//! 认出是那一家以后，地址还得是官方的才交 key（PK-1 补，2026-11-16 语音会话报：catalog 写了 deepseek、地址指中转站的，中转站
//! 的 key 被交给扩展、发到了官方地址）：没写地址（照档案就是官方的）或者字面地址的主机就是 [`OFFICIAL`] 那一个才算；地址是
//! 环境变量的引用（查不出来）、别的地址一律不算。catalog 只说「模型照这一家的档案」，不说明 key 是这一家官方的。
//!
//! 几家都对上的照编号排第一的那一家；没写 key 的不算。

use miyu_config::secret::Reference;
use miyu_config::{Address, Values};

use crate::provider::configured;
use crate::settings::ProviderSettings;

/// 几家的官方地址：目录里的编号到地址里认的那一段。只放实测过的。
pub const OFFICIAL: [(&str, &str); 1] = [("deepseek", "api.deepseek.com")];

/// 没写时借模型那边供应商的 key 的配置项（施工 NS-1，项目主人 2026-11-17 定「两处都留，优先用网络搜索里填的；没填、有
/// 官方的 DeepSeek 供应商的借它，显示为已继承」）：配置项的键到目录里的那一家。联网包照它借（`miyu-net` 的 `settings.rs`），
/// `config.get` 照它写来源 `inherited`（`miyu-endpoint` 的 `config/inherited.rs`），一处登记两边都认。
pub const INHERITED: [(&str, &str); 1] = [("net.search.deepseek.key", "deepseek")];

/// 配置项 `key` 没写时借的那一家供应商：编号和 key 的引用（[`serving`]）。不在 [`INHERITED`] 里的、没有能借的没有。只看借得到
/// 借不到，`key` 自己写没写由调的一方看。
pub fn inherited(values: &Values, key: &str) -> Option<(String, Reference)> {
    let (_, catalog) = INHERITED.iter().find(|(item, _)| *item == key)?;
    serving(values, catalog)
}

/// 配置 `values` 里算目录里 `catalog` 那一家、写了 key 的第一家供应商（照编号排）：编号和 key 的引用。没有的没有。
pub fn serving(values: &Values, catalog: &str) -> Option<(String, Reference)> {
    let official = official_host(catalog);
    configured(values).into_iter().find_map(|id| {
        let settings = ProviderSettings::at(values, &[&id]);
        let key = settings.key?;
        let is_catalog = match (&settings.catalog, &settings.base_url) {
            (Some(written), _) => written == catalog,
            (None, Some(Address::Literal(url))) => official.is_some_and(|host| host_is(url, host)),
            (None, Some(Address::Env(_))) => false,
            (None, None) => id == catalog,
        };
        let official = official_address(settings.base_url.as_ref(), catalog);
        (is_catalog && official).then_some((id, key))
    })
}

/// 目录里 `catalog` 那一家的官方地址认的那一段（[`OFFICIAL`]）；没列的没有。
pub fn official_host(catalog: &str) -> Option<&'static str> {
    OFFICIAL
        .iter()
        .find(|(id, _)| *id == catalog)
        .map(|(_, host)| *host)
}

/// 配置里一家供应商写的地址 `written` 算不算 `catalog` 那一家官方的：没写的算（照档案就是官方的），字面地址的主机是
/// [`OFFICIAL`] 里那一个的算；环境变量的引用（查不出来）、别的地址不算。交 key（[`serving`]）、查余额（施工 QU-2，
/// [`crate::quota::settled`]）都照它。
pub fn official_address(written: Option<&Address>, catalog: &str) -> bool {
    match written {
        None => true,
        Some(Address::Literal(url)) => {
            official_host(catalog).is_some_and(|host| host_is(url, host))
        }
        Some(Address::Env(_)) => false,
    }
}

/// 地址 `url` 的主机是不是 `host`：去掉协议、用户名、端口、路径以后整段相同（不分大小写）；路径、查询里出现 `host` 的不算。
fn host_is(url: &str, host: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, after)| after);
    let name = authority.split(':').next().unwrap_or_default();
    name.eq_ignore_ascii_case(host)
}

#[cfg(test)]
mod tests;
