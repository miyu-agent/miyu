//! 「已继承」的来源（施工 NS-1，项目主人 2026-11-17 定）：联网包的 DeepSeek 的 key 没写、模型那边有官方的 DeepSeek 供应商的，
//! 搜索照借它的 key（`miyu-net` 的 `settings.rs`）。`config.get`、`config.set` 的回应和推送里这一项的来源不写 `default`，写
//! `{"layer": "inherited", "from": "providers.<编号>.key"}`，头照它画「已继承」；值照旧是 `null`，不交 key。借哪几项、借谁照
//! `miyu_models::provider_key::INHERITED`，认法照 `serving`（官方地址才借，施工 PK-1补）。

use serde_json::{Value, json};

use miyu_config::Values;
use miyu_config::merge::Origin;

/// 配置项 `key` 哪儿都没写（`origin` 是没有，或者出厂的），又借得到供应商的 key 的：写成「已继承」。别的没有，照原来的写法。
/// 没有出厂值的项哪儿都没写时合出来是没有，`config.get` 原来不列它；借得到的照样列，值是 `null`。
pub(super) fn origin(key: &str, origin: Option<&Origin>, values: &Values) -> Option<Value> {
    if !matches!(origin, None | Some(Origin::Default)) {
        return None;
    }
    let (provider, _) = miyu_models::provider_key::inherited(values, key)?;
    Some(json!({"layer": "inherited", "from": format!("providers.{provider}.key")}))
}

#[cfg(test)]
mod tests;
