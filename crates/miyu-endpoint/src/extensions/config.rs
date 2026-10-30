//! 包自己的配置（施工 9-4 下下，`docs/blueprint/extensions.md`「配置」）：核心拉起的扩展握手时交给它，之后变了推
//! `extension.config`。是 `config.md` 第九条「密钥不经协议交出去」的例外：只在核心亲手拉起、走标准输入输出的连接上，只给
//! 这个包自己的键（清单里声明的那几项，键照前缀起，施工 PN-1），值不进运行日志、不进事件。

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::config::Config;

/// 包 `package` 自己的那份：键到值。值是最终值（没写的照默认值，配置合出来的，不看项目配置），密钥引用解成
/// 真值的字；没设、没默认值、引用取不到的那一键不放。
pub(crate) fn own(config: &Config, package: &str) -> BTreeMap<String, Value> {
    let resolved = config.resolved();
    config
        .items()
        .iter()
        .filter(|item| declared_by(item, package))
        .filter_map(|item| {
            let (value, _) = resolved.get(item.key)?;
            Some((item.key.to_string(), handed(config, item.kind, value)?))
        })
        .collect()
}

/// 交出去的一个值：密钥引用解成真值的字（取不到的没有）；一组字段的列表（施工 PC-2）每一项是对象，密钥字段解成真值、取不到的
/// 那个字段不放，没写的字段照默认值，没有默认值的不放。
fn handed(config: &Config, kind: miyu_config::Kind, value: &miyu_config::Value) -> Option<Value> {
    match (kind, value) {
        (_, miyu_config::Value::Secret(reference)) => Some(Value::String(
            config.secret(reference)?.expose().to_string(),
        )),
        (
            miyu_config::Kind::List(miyu_config::Kind::Record(record)),
            miyu_config::Value::List(items),
        ) => Some(Value::Array(
            items
                .iter()
                .map(|item| {
                    let miyu_config::Value::Record(fields) = item else {
                        return Value::Object(Map::new());
                    };
                    Value::Object(
                        record
                            .fields
                            .iter()
                            .filter_map(|field| {
                                let value = fields.get(field.key).or(field.default.as_ref())?;
                                Some((field.key.to_string(), handed(config, field.kind, value)?))
                            })
                            .collect(),
                    )
                })
                .collect(),
        )),
        (_, other) => Some(other.json()),
    }
}

/// 配置项 `item` 是不是包 `package` 的清单里声明的（施工 PN-1）：键照前缀起，不一定以包编号开头，照挂在哪个包认；「软件包」
/// 页上关掉这个包的那一项（`packages.<编号>.off`）也挂在它底下，是核心的，不算。
pub(crate) fn declared_by(item: &miyu_config::Item, package: &str) -> bool {
    item.ui.package == Some(package)
        && !item.key.contains('<')
        && item.key != miyu_config::key::fill(miyu_session::packages_off::OFF, &[package])
}

/// 编号是 `package` 的包的前缀（施工 PN-1，设计 48 第二节）：照核心读到的清单；没读成的、没装的照包编号。
pub(crate) fn prefix_of(core: &crate::Core, package: &str) -> String {
    core.system_packages()
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok())
        .map_or(package, |manifest| manifest.prefix_of(package))
        .to_string()
}

/// 从 `before` 到 `after` 变了的键：新值，没了的是 `null`。没变的不放；都没变的是空的。
pub(crate) fn changes(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> Map<String, Value> {
    let mut changed = Map::new();
    for key in before.keys().chain(after.keys()) {
        let now = after.get(key);
        if now != before.get(key) && !changed.contains_key(key) {
            changed.insert(key.clone(), now.cloned().unwrap_or(Value::Null));
        }
    }
    changed
}

#[cfg(test)]
mod tests;
