//! `model.list` 里一家供应商真要哪几项（施工 FX-11，`internal/blueprint/models.md`「协议」`model.list` 的 `needs`）：头照它只画这家
//! 真有的项。软件包带的供应商（Antigravity 这种扩展驱动）没有地址、不要密钥，头原来照内置驱动的样子把 ID、显示名、Base URL、
//! 接口、认证、API 密钥都画出来。
//!
//! - `key`、`base_url` 各是 `none`、`optional`、`required` 之一。扩展驱动照它清单 `[[driver]]` 写的 `key`、`base_url`；扩展装着、
//!   驱动名对上了才知道，没对上的不写 `needs`（这一家本来就用不了，`problem` 说了）。
//! - 内置驱动：`key` 是 `required`，本机的服务（配置写了 `local = true`，或者地址在本机回环上）是 `optional`；`base_url` 是
//!   `required`，目录里认出了这一家（`catalog`）的是 `optional`：没写的照目录推。
//! - 这一家是软件包带的（`[providers.<编号>]`，施工 PV-4）：多一格 `package`，包的编号。头把包给的值画成暗字，人照样能改。

use serde_json::{Value, json};

use miyu_config::Values;
use miyu_config::merge::{Origin, Resolved};
use miyu_config::package::DriverKey;
use miyu_models::provider::Driver;
use miyu_models::settings::ProviderSettings;
use miyu_store::packages::Found;

/// 给 `providers` 每一项补上 `needs`、`package`：`packages` 是装着的包，照它找扩展驱动的声明。
pub(super) fn stamp(providers: &mut [Value], resolved: &Resolved, packages: &[Found]) {
    let values = resolved.values();
    for entry in providers {
        let Some(id) = entry["id"].as_str().map(str::to_string) else {
            continue;
        };
        if let Some(needs) = needs(entry, &values, &id, packages) {
            entry["needs"] = needs;
        }
        let key = miyu_config::key::fill("providers.<id>.driver", &[&id]);
        if let Some((_, Origin::Package(package))) = resolved.get(&key) {
            entry["package"] = json!(package);
        }
    }
}

/// 这一家真要的：`{"key", "base_url"}`。驱动是扩展的、又找不到声明的是空的。
fn needs(entry: &Value, values: &Values, id: &str, packages: &[Found]) -> Option<Value> {
    let settings = ProviderSettings::at(values, &[id]);
    let driver = entry["driver"].as_str().or(settings.driver.as_deref())?;
    match Driver::parse(driver)? {
        Driver::Extension(reference) => {
            let declared = packages
                .iter()
                .find(|found| found.id == reference.package)
                .and_then(|found| found.read.as_ref().ok())
                .and_then(|manifest| {
                    manifest
                        .drivers
                        .iter()
                        .find(|one| one.name == reference.name)
                })?;
            Some(json!({"key": said(declared.key), "base_url": said(declared.base_url)}))
        }
        _ => {
            let local = settings.local.unwrap_or_else(|| {
                entry["base_url"]
                    .as_str()
                    .is_some_and(miyu_http::is_loopback_url)
            });
            let key = if local { "optional" } else { "required" };
            let base_url = if entry.get("catalog").is_some() {
                "optional"
            } else {
                "required"
            };
            Some(json!({"key": key, "base_url": base_url}))
        }
    }
}

/// 三种写法。
fn said(need: DriverKey) -> &'static str {
    match need {
        DriverKey::None => "none",
        DriverKey::Optional => "optional",
        DriverKey::Required => "required",
    }
}

#[cfg(test)]
mod tests;
