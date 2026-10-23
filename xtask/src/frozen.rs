//! 协议版本 1 冻结（施工 Y-5 末下，Y-5 施工单第五节第 4 条）：协议的方法——头和扩展调核心的、核心发给扩展的——在
//! `docs/reference/methods-v1.txt` 里一行一个，照字母排。冻结以后只加不改不删：代码里没了快照里还有的（删了、改了名）报红，
//! 破坏性的改动只能升到协议 2（握手照 `[最低, 最高]` 谈）；代码里新加的也报，要登记进快照（`cargo xtask methods` 只往里加，
//! 不删）。算在「文档」那一项里。找方法照协议参考那一套（`reference.rs`，连核心发出去的一起）。

use std::collections::BTreeSet;
use std::path::Path;

use crate::reference;

/// 快照在仓库里的位置。
pub const PATH: &str = "docs/reference/methods-v1.txt";

/// 快照开头的说明，`#` 开头的行不算方法。
const HEADER: &str = "# 协议版本 1 冻结的方法（施工 Y-5 末下）：只加不改不删。删、改名、改意思的要升到协议 2。\n# Methods frozen in protocol 1: add only; removing, renaming or changing one needs protocol 2.\n";

/// 代码里分发的、核心发出去的方法。
fn served(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut code = BTreeSet::new();
    for dir in reference::SOURCES {
        reference::scan_dir(&root.join(dir), &mut code, true)
            .map_err(|e| format!("读不了 {dir}：{e}"))?;
    }
    Ok(code)
}

/// 快照里的方法，照写的先后；`#` 开头的、空的行不算。
fn listed(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// 快照 `frozen` 对代码里的 `code`：要改的地方。
fn problems(frozen: &[String], code: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut sorted = frozen.to_vec();
    sorted.sort();
    sorted.dedup();
    if sorted != frozen {
        problems.push(format!("{PATH}：照字母排、一个只写一次"));
    }
    let frozen: BTreeSet<&String> = frozen.iter().collect();
    for name in frozen.iter().filter(|name| !code.contains(name.as_str())) {
        problems.push(format!(
            "{PATH}：协议 1 冻结了 {name}，代码里没了：不能删、不能改名；破坏性的改动要升到协议 2"
        ));
    }
    for name in code.iter().filter(|name| !frozen.contains(name)) {
        problems.push(format!(
            "{PATH}：代码里新加了 {name}：协议 1 只加不改，登记进快照（cargo xtask methods）"
        ));
    }
    problems
}

/// 查快照，交回问题。
pub fn check(root: &Path) -> Vec<String> {
    let code = match served(root) {
        Ok(code) => code,
        Err(e) => return vec![e],
    };
    match std::fs::read_to_string(root.join(PATH)) {
        Ok(text) => problems(&listed(&text), &code),
        Err(e) => vec![format!("读不了 {PATH}：{e}")],
    }
}

/// `cargo xtask methods`：把代码里新加的方法登记进快照（只加，快照里有的一个都不删），交回加了几个。
///
/// # Errors
///
/// 代码、快照读不了，写不进。
pub fn add(root: &Path) -> Result<usize, String> {
    let code = served(root)?;
    let path = root.join(PATH);
    let mut all: BTreeSet<String> = match std::fs::read_to_string(&path) {
        Ok(text) => listed(&text).into_iter().collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeSet::new(),
        Err(e) => return Err(format!("读不了 {PATH}：{e}")),
    };
    let before = all.len();
    all.extend(code);
    let body: String = all.iter().map(|name| format!("{name}\n")).collect();
    std::fs::write(&path, format!("{HEADER}{body}")).map_err(|e| format!("写不进 {PATH}：{e}"))?;
    Ok(all.len() - before)
}

#[cfg(test)]
mod tests;
