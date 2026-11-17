//! 配置的包这一层（施工 PV-4，`docs/blueprint/config.md`「最终值和来源」，设计 `66-扩展提供模型驱动.md` 第四节第 2 条，项目主人
//! 2026-11-17 定）：装着、开着的包清单里 `[providers.<编号>]` 声明的供应商，合在默认值之上、配置文件之下，不写进配置文件；卸掉、
//! 关掉就没有。
//!
//! 一家包带的供应商 `X`（包 `P` 带的）和配置文件怎么对：
//!
//! - 配置文件里没有 `providers.X.*` 的：照包给的。
//! - 配置文件里有、写着 `providers.X.package = "P"` 的：人改过包带的这一家，写的几项盖在包给的上面（`config.set` 改包带的这一家
//!   时顺手写上它，`config/set.rs`）。
//! - 配置文件里有、没写 `package`（或者写的是别的包）的：是人自己的供应商，人的优先，包带的这一家整家不出现，`miyu check` 提示
//!   （[`Clash::Shadowed`]）。不照「写没写驱动」猜：人原来照目录推驱动、只写了 key 的那一家，不能让新装的包接走 key（设计 66
//!   第九节第 2 条）。
//! - 配置文件里写着 `package = "P"`、`P` 这时不带 `X`（卸了、关了）的：那一段整段不算（[`miyu_config::merge::Packaged::masked`]），
//!   照旧写在文件里，包回来了接着用。
//!
//! 两个包带了同一个编号：照包的先后（同 `Core::packages_of`）先到先得，后到的不出现（[`Clash::Taken`]）。驱动指别的包的扩展驱动的
//! 整家不收（[`Clash::Foreign`]）；照配置清单读不过的那几格不收（[`Clash::Invalid`]）。

use std::collections::{BTreeMap, BTreeSet};

use miyu_config::merge::Packaged;
use miyu_config::package::ProviderDecl;
use miyu_config::parse::{Parsed, parse};
use miyu_config::{Item, Layer, Value};

/// 包带的一家没出现、没全收的原因：`miyu check` 照它提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Clash {
    /// 配置文件里有人自己的同编号供应商：包带的那一家不出现。
    Shadowed {
        /// 供应商编号。
        provider: String,
        /// 带它的包。
        package: String,
    },
    /// 先到的包已经带了这个编号。
    Taken {
        /// 供应商编号。
        provider: String,
        /// 后到、没收的包。
        package: String,
        /// 先到的包。
        by: String,
    },
    /// 驱动指的是别的包的扩展驱动：整家不收。
    Foreign {
        /// 供应商编号。
        provider: String,
        /// 带它的包。
        package: String,
    },
    /// 照配置清单读不过的格：那几格不收，别的照收。
    Invalid {
        /// 供应商编号。
        provider: String,
        /// 带它的包。
        package: String,
        /// 读不过的键（真的键，或者问题的原因码）。
        keys: Vec<String>,
    },
}

/// 一家：哪个包带的、清单里的样子。
pub(crate) type Declared = (String, ProviderDecl);

/// 包带的几家 `declared`（照包的先后）照配置清单 `items` 读成包这一层，名字照 `language` 挑；`system` 是配置文件读成的样子。
pub(crate) fn layer(
    items: &[Item],
    declared: &[Declared],
    system: &Parsed,
    language: &str,
) -> (Packaged, Vec<Clash>) {
    let mut packaged = Packaged::default();
    let mut clashes = Vec::new();
    let mut taken: BTreeMap<&str, &str> = BTreeMap::new();
    for (package, decl) in declared {
        let provider = decl.id.clone();
        if decl.driver_package().is_some_and(|owner| owner != package) {
            clashes.push(Clash::Foreign {
                provider,
                package: package.clone(),
            });
            continue;
        }
        if let Some(by) = taken.get(decl.id.as_str()) {
            clashes.push(Clash::Taken {
                provider,
                package: package.clone(),
                by: (*by).to_string(),
            });
            continue;
        }
        if owned_by_person(system, &decl.id, package) {
            clashes.push(Clash::Shadowed {
                provider,
                package: package.clone(),
            });
            continue;
        }
        taken.insert(&decl.id, package);
        let read = read_one(items, decl, language);
        let invalid: Vec<String> = read
            .problems
            .iter()
            .map(|problem| {
                problem
                    .key
                    .clone()
                    .unwrap_or_else(|| problem.code.as_str().to_string())
            })
            .collect();
        if !invalid.is_empty() {
            clashes.push(Clash::Invalid {
                provider,
                package: package.clone(),
                keys: invalid,
            });
        }
        for (key, entry) in read.entries {
            packaged.owners.insert(key.clone(), package.clone());
            packaged.parsed.entries.insert(key, entry);
        }
    }
    packaged.masked = orphans(system, &taken);
    (packaged, clashes)
}

/// 人这一次改的几个键 `keys` 里，落在包带的那一家上、配置文件里还没写 `package` 的：那一家要顺手写上的 `(键, 包)`（`config.set`
/// 改包带的那一家，施工 PV-4）。照编号排、一家一条。
pub(crate) fn markers<'a>(
    config: &super::Config,
    keys: impl Iterator<Item = &'a str>,
) -> Vec<(String, String)> {
    let mut found = BTreeMap::new();
    for key in keys {
        let Some(id) = key
            .strip_prefix("providers.")
            .and_then(|rest| rest.split('.').next())
        else {
            continue;
        };
        let start = prefix(id);
        let Some((_, package)) = config
            .packaged
            .owners
            .iter()
            .find(|(owned, _)| owned.starts_with(&start))
        else {
            continue;
        };
        let marker = format!("{start}package");
        if marker != key && !config.system.parsed.entries.contains_key(&marker) {
            found.insert(marker, package.clone());
        }
    }
    found.into_iter().collect()
}

/// 配置文件里 `providers.<id>` 那一段的前缀。
fn prefix(id: &str) -> String {
    format!("providers.{id}.")
}

/// 配置文件里有 `providers.<id>.*`、又没写 `package = "<package>"`：人自己的那一家。
fn owned_by_person(system: &Parsed, id: &str, package: &str) -> bool {
    let start = prefix(id);
    let written = system.entries.keys().any(|key| key.starts_with(&start));
    written && marker(system, id).as_deref() != Some(package)
}

/// 配置文件里 `providers.<id>.package` 写的包。
fn marker(system: &Parsed, id: &str) -> Option<String> {
    match &system
        .entries
        .get(&format!("providers.{id}.package"))?
        .value
    {
        Value::Text(package) => Some(package.to_string()),
        _ => None,
    }
}

/// 配置文件里写着 `package = "<P>"`、`P` 这时不带它的那几段的全部键。
fn orphans(system: &Parsed, taken: &BTreeMap<&str, &str>) -> BTreeSet<String> {
    let ids: BTreeSet<&str> = system
        .entries
        .keys()
        .filter_map(|key| key.strip_prefix("providers.")?.strip_suffix(".package"))
        .filter(|id| !id.contains('.'))
        .collect();
    let mut masked = BTreeSet::new();
    for id in ids {
        let written = marker(system, id);
        if written.is_some() && taken.get(id).copied() != written.as_deref() {
            let start = prefix(id);
            masked.extend(
                system
                    .entries
                    .keys()
                    .filter(|key| key.starts_with(&start))
                    .cloned(),
            );
        }
    }
    masked
}

/// 一家照配置清单读：清单里那一张，加上照语言挑的名字。读不过的格不收（`parse` 只丢写错的那一项）。
fn read_one(items: &[Item], decl: &ProviderDecl, language: &str) -> Parsed {
    let mut text = decl.table.clone();
    if let Some(name) = crate::personas::pick(&decl.name, language) {
        let header = format!("[providers.{}]\n", decl.id);
        // 名字照 JSON 的写法转义：JSON 的字符串也是 TOML 的基本字符串。
        let line = format!(
            "name = {}\n",
            serde_json::to_string(&name).unwrap_or_default()
        );
        text = match text.strip_prefix(&header) {
            Some(rest) => format!("{header}{line}{rest}"),
            None => format!("{text}\n{header}{line}"),
        };
    }
    parse(items, Layer::System, &text).unwrap_or_else(|problem| Parsed {
        entries: BTreeMap::new(),
        problems: vec![*problem],
    })
}

#[cfg(test)]
mod tests;
