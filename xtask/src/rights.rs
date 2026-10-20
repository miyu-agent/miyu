//! 扩展能调什么的表（施工 X-30，`docs/blueprint/extensions.md`「谁能调」）：代码里分发的每个方法都要在
//! `crates/miyu-endpoint/src/connection/rights.rs` 的表里写明扩展能不能调；表里写了、代码里没有的也报。算在「文档」那一项里。
//!
//! 代码这一头照协议参考那一套找（`reference.rs`），不算核心发出去的推送；表这一头认 `rights.rs` 里样子是方法名的字符串字面量。
//! 没带点的几个（`hello`、`subscribe`、`provide`、`unsubscribe`）两头都不认，表里照样写着。

use std::collections::BTreeSet;
use std::path::Path;

use crate::reference;

/// 表在仓库里的位置。
pub const PATH: &str = "crates/miyu-endpoint/src/connection/rights.rs";

/// 查表，交回问题。
pub fn check(root: &Path) -> Vec<String> {
    let mut served = BTreeSet::new();
    for dir in reference::SOURCES {
        if let Err(e) = reference::scan_dir(&root.join(dir), &mut served, false) {
            return vec![format!("读不了 {dir}：{e}")];
        }
    }
    let table = match std::fs::read_to_string(root.join(PATH)) {
        Ok(text) => listed(&text),
        Err(e) => return vec![format!("读不了 {PATH}：{e}")],
    };
    let mut problems = Vec::new();
    for name in served.difference(&table) {
        problems.push(format!(
            "{PATH}：代码里分发的 {name} 不在表里，写明扩展能不能调（只给人、要哪项能力、都能调）"
        ));
    }
    for name in table.difference(&served) {
        problems.push(format!("{PATH}：表里的 {name} 代码里没有分发"));
    }
    problems
}

/// 表里写着的方法名：测试以外的字符串字面量里样子是方法名的。
fn listed(text: &str) -> BTreeSet<String> {
    let text = text.split("#[cfg(test)]").next().unwrap_or_default();
    text.split('"')
        .skip(1)
        .step_by(2)
        .filter(|literal| reference::is_method(literal))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests;
