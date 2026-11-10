//! `config.schema` 里包的分节（施工 SG-1，`docs/blueprint/config.md`「协议」、`packages.md`「配置项」第 1 条）：组里的项带
//! `section`（组名），这个包那一组带 `sections`（每一节的编号、名字、`shown_when`，照清单写的先后）。照装着的包的清单现拼，
//! 名字照连接的语言挑，同一项的名字。没分组的包两样都不写，`config.schema` 的字节和原来一样。
//!
//! 只管怎么画：值照旧存、读、查，核心哪里都不看 `shown_when`。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use miyu_config::package::Manifest;
use miyu_config::phrases;

use crate::Core;

/// 装着的包的分节：项的键到组名，包的编号到它那一组的 `sections`。
pub(super) struct Sections {
    of_item: BTreeMap<String, String>,
    of_package: BTreeMap<String, Value>,
}

impl Sections {
    /// 照核心手里装着的包拼，名字照语言 `language` 挑。
    pub(super) fn new(core: &Core, language: &str) -> Sections {
        let packages = core.system_packages();
        let mut sections = Sections {
            of_item: BTreeMap::new(),
            of_package: BTreeMap::new(),
        };
        for (id, manifest) in crate::packages::manifests(&packages) {
            if manifest.sections.is_empty() {
                continue;
            }
            let prefix = manifest.prefix_of(id);
            for setting in &manifest.settings {
                if let Some(section) = setting.section() {
                    let key = format!("{prefix}.{}", setting.name);
                    sections.of_item.insert(key, section.to_string());
                }
            }
            let listed = listed(manifest, prefix, language);
            sections.of_package.insert(id.to_string(), listed);
        }
        sections
    }

    /// 键 `key` 那一项在哪一节；不在包的组里的没有。
    pub(super) fn item(&self, key: &str) -> Option<&str> {
        self.of_item.get(key).map(String::as_str)
    }

    /// 包 `package` 那一组的 `sections`；没分组的没有。
    pub(super) fn package(&self, package: &str) -> Option<&Value> {
        self.of_package.get(package)
    }
}

/// 一个包的每一节：没写名字的照编号；`shown_when` 的键写全（前缀加清单里的名字），值总是列表。
fn listed(manifest: &Manifest, prefix: &str, language: &str) -> Value {
    let sections: Vec<Value> = manifest
        .sections
        .iter()
        .map(|section| {
            let name = phrases::pick(&section.name, language).unwrap_or(&section.id);
            let mut one = json!({"id": section.id, "name": name});
            if let Some(when) = &section.shown_when {
                let key = format!("{prefix}.{}", when.setting);
                one["shown_when"] = json!({"key": key, "is": when.values});
            }
            one
        })
        .collect();
    Value::Array(sections)
}
