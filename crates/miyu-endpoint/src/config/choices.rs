//! `config.schema` 里选项由包在运行时给的项（施工 SG-2，`docs/blueprint/config.md`「协议」、`packages.md`「配置项」的表）：清单写了
//! `choices_from` 的文字项带 `choices_from: {"package", "method"}`，头调 `package.call` 向那个包要选项、画成下拉。照装着的包的
//! 清单现拼；没写的项不带这一格，`config.schema` 的字节和原来一样。
//!
//! 只管怎么画：核心不调那个方法、不照它回的列表查值，值照旧当普通文字存、读、查。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::Core;

/// 装着的包里写了 `choices_from` 的项：项的键到 `{"package", "method"}`。
pub(super) struct Choices(BTreeMap<String, Value>);

impl Choices {
    /// 照核心手里装着的包拼。
    pub(super) fn new(core: &Core) -> Choices {
        let packages = core.system_packages();
        let mut choices = BTreeMap::new();
        for (id, manifest) in crate::packages::manifests(&packages) {
            let prefix = manifest.prefix_of(id);
            for setting in &manifest.settings {
                if let Some(method) = &setting.choices_from {
                    let key = format!("{prefix}.{}", setting.name);
                    choices.insert(key, json!({"package": id, "method": method}));
                }
            }
        }
        Choices(choices)
    }

    /// 键 `key` 那一项的选项向谁要；清单没写的没有。
    pub(super) fn item(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
}
