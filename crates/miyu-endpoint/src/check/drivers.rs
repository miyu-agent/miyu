//! 供应商写了扩展提供的驱动的（施工 MD-1，设计 66 第四节第 5 条、第九节第 3 条）：配置这一层已经管了写法和「装没装、驱动名对不对」
//! （不在选项里的记警告 `unknown_driver`，`miyu_config::driver_ref`），这里接着查装着的那几个：
//!
//! - 包没批 `model.driver`（`driver_unapproved`，错误）：这家供应商用不了；
//! - 驱动声明 `key = "required"`、这家没配 key（`driver_key_missing`，错误）；
//! - 驱动声明 `key = "none"`、这家却配了 key（`driver_key_unused`，警告）：不交给它。

use serde_json::{Value, json};

use miyu_config::Words;
use miyu_config::package::{Capability, DriverKey};
use miyu_models::provider::Driver;
use miyu_models::settings::ProviderSettings;
use miyu_store::human::Human;

use super::shown;
use crate::Core;

/// 几条：`kind` 是 `config`，`file` 是系统配置。
pub(super) fn drivers(core: &Core, words: &Human) -> Vec<Value> {
    let resolved = core.config_now().borrow().resolved().clone();
    let values = resolved.values();
    let packages = core.system_packages();
    let file = shown(core, &core.root.config_file());
    let say = |code: &str, level: &str, fields: &[(&str, &str)]| {
        let message =
            Words::sentence(words, &format!("package-problems/{code}"), fields).unwrap_or_default();
        json!({"kind": "config", "file": file, "code": code, "level": level, "message": message})
    };
    let mut problems = Vec::new();
    for id in miyu_config::key::names(values.keys(), "providers.<id>", &[]) {
        let settings = ProviderSettings::at(&values, &[&id]);
        let Some(Driver::Extension(reference)) = settings.driver.as_deref().and_then(Driver::parse)
        else {
            continue;
        };
        let written = reference.to_string();
        let Some(declared) = packages
            .iter()
            .find(|found| found.id == reference.package)
            .and_then(|found| found.read.as_ref().ok())
            .and_then(|manifest| {
                manifest
                    .drivers
                    .iter()
                    .find(|driver| driver.name == reference.name)
            })
        else {
            // 没装、驱动名写错了：配置那一层报过 `unknown_driver`。
            continue;
        };
        let fields = [("provider", id.as_str()), ("driver", written.as_str())];
        if !crate::extensions::held(core, &reference.package).contains(&Capability::ModelDriver) {
            problems.push(say("driver_unapproved", "error", &fields));
        }
        match (declared.key, settings.key.is_some()) {
            (DriverKey::Required, false) => {
                problems.push(say("driver_key_missing", "error", &fields))
            }
            (DriverKey::None, true) => problems.push(say("driver_key_unused", "warning", &fields)),
            _ => {}
        }
    }
    problems
}
