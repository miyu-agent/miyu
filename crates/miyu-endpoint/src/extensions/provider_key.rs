//! 扩展拿主人配的某一家模型供应商的 key（施工 PK-1）：清单 `[provider_key] catalog` 写要哪一家，声明 `provider.key`，人批过
//! （出厂的也要人批，[`Capability::needs_person`]）。核心交给它的配置（握手的 `config`、`extension.config`，`config.rs`）里多一格
//! [`CELL`]：照 `miyu_models::provider_key::serving` 认出来的那一家供应商的 key 的真值。和包自己的密钥一样是 `config.md`
//! 第九条「密钥不经协议交出去」的例外，只走核心亲手拉起的那条连接，值不进运行日志、不进事件；`config.get` 照旧只交引用。
//!
//! 不交的：没批 `provider.key` 的，清单没写 `[provider_key]` 的，配置里没有那一家、那一家没写 key、key 取不到的。别家供应商的 key
//! 一律不交。换了 key、删了那一家，照配置推送的规矩推新值或者 `null`。

use miyu_config::package::Capability;

use crate::Core;
use crate::config::Config;

/// 交给扩展的配置里那一格的名字：包自己的配置项都是 `<前缀>.<名字>`，不带 `.` 的这一格撞不上。
pub(crate) const CELL: &str = "provider_key";

/// 照配置 `config` 交给包 `package` 的那一家供应商的 key 的真值；不交的（见模块说明）没有。
pub(crate) fn handed(core: &Core, config: &Config, package: &str) -> Option<String> {
    let packages = core.system_packages();
    let manifest = packages
        .iter()
        .find(|found| found.id == package)?
        .read
        .as_ref()
        .ok()?;
    let wanted = manifest.provider_key.as_ref()?;
    let switches = super::read_switches(core).0;
    if !super::approval::holds(core, package, manifest, &switches)
        .contains(&Capability::ProviderKey)
    {
        return None;
    }
    let values = config.resolved().values();
    let (_, reference) = miyu_models::provider_key::serving(&values, &wanted.catalog)?;
    Some(config.secret(&reference)?.expose().to_string())
}

/// 目录里编号 `catalog` 那一家给人看的名字（装包那一屏说「会拿到你 <名字> 的 key」）：照目录里的，目录还没读完、没有这一家的照
/// 编号。
pub(crate) fn shown_name(core: &Core, catalog: &str) -> String {
    core.model_data
        .with_once(|knowledge| {
            knowledge
                .lookup
                .and_then(|lookup| lookup.provider_side(catalog, None, Some(catalog)))
                .and_then(|side| side.name)
        })
        .unwrap_or_else(|| catalog.to_string())
}
