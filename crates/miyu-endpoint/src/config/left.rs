//! 已经卸掉的包留在配置里的设置项（施工 PK-5补，`docs/blueprint/packages.md`「装卸」第 7 条）：照 pacman，不带 `purge` 卸包
//! 时 `etc/config.toml` 里它的设置项原样留着。核心读配置时这样认、这样跳过：
//!
//! - 认：手里有一份「前缀 → 留下它的包」（[`Left`]）。来处两样（`crate::packages::settings_left::left`）：卸掉的出厂包照资源目录里
//!   的清单（声明了设置项的才算），你装的包照 `etc/leftover/<编号>.toml` 记的前缀；这时装着的编号不算。核心起来时交进来
//!   （[`super::Config::load_leaving`]），装卸以后照新的清单一起换（`refit`）。
//! - 跳过：解析照旧（不认识的键本来就不进最终值，不生效），解析出的 `unknown_key` 里，键的第一段是留下的前缀的拿掉、记到
//!   这份文件的 `leftover` 里：不算问题（不进 `config.get`、`config.changed` 的问题，不进起来时 `WARN config problems` 的计数）。
//!   别的不认识的键照报；前缀被这时装着的包占着的，照那个包的配置项认，根本不是不认识的。
//! - `check` 照拿掉的数一数，每个包一条警告 `settings_left`（`check.rs`）。

use std::collections::BTreeMap;

use miyu_config::parse::Parsed;
use miyu_config::problem::Code;

/// 拿掉的留下的键：键和留下它的包，照文件里的先后。
pub(crate) type Taken = Vec<(String, String)>;

/// 留下的设置项的前缀到留下它的包的编号。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Left(BTreeMap<String, String>);

impl Left {
    /// 照 `(前缀, 编号)` 拼一份；同一个前缀先到的算。
    pub fn new(pairs: impl IntoIterator<Item = (String, String)>) -> Left {
        let mut map = BTreeMap::new();
        for (prefix, id) in pairs {
            map.entry(prefix).or_insert(id);
        }
        Left(map)
    }

    /// 真的键 `key` 是哪个已卸的包留下的：第一段是留下的前缀的，是它；别的没有。
    pub(crate) fn owner(&self, key: &str) -> Option<&str> {
        let first = miyu_config::key::split(key)?.into_iter().next()?;
        self.0.get(&first).map(String::as_str)
    }

    /// 从 `parsed` 的问题里拿掉留下的键报的 `unknown_key`，交回拿掉的：键和留下它的包。
    pub(crate) fn take(&self, parsed: &mut Parsed) -> Taken {
        if self.0.is_empty() {
            return Vec::new();
        }
        let mut taken = Vec::new();
        parsed.problems.retain(|problem| {
            let owner = (problem.code == Code::UnknownKey)
                .then_some(problem.key.as_deref())
                .flatten()
                .and_then(|key| Some((key, self.owner(key)?)));
            match owner {
                Some((key, id)) => {
                    taken.push((key.to_string(), id.to_string()));
                    false
                }
                None => true,
            }
        });
        taken
    }
}

/// 拿掉的那几个照包数一数：编号到几项，照编号排。
pub(crate) fn counted(taken: &[(String, String)]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for (_, id) in taken {
        *counts.entry(id.clone()).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests;
