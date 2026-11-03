//! 包装上以后模型会多哪些工具（施工 PV-1）：「下载软件」的详情页照它写一行「提供」。软件源索引的每一条、`package.info` 的回应都带
//! 这一格（`tools`），收录、核心读本机包都调 [`of`]。
//!
//! - 脚本：包文件夹 `scripts/` 下那一层的普通文件，照开头读（[`miyu_policy::script::read`]，核心登记脚本用的同一份），工具名、
//!   显示名就是她看到的、界面上写的。点开头的、子目录、链接不看（收录的是别人的包，不顺着链接读到包外面去），读不成的不算。
//! - 扩展：清单里各功能写明的工具名（`[features.<编号>] tools`），没有显示名。MCP 的、要跑起来才知道的不列。
//!
//! 照名字排，同名的只留一个（脚本在前）。

use std::collections::BTreeMap;
use std::path::Path;

use miyu_config::package::Manifest;
use serde::{Deserialize, Serialize};

/// 一件工具。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    /// 模型看到的工具名。
    pub name: String,
    /// 给人看的名字，照语言（`zh`、`en`）；没写的语言不出。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub title: BTreeMap<String, String>,
    /// 从哪来：`script`、`extension`。
    pub from: String,
}

/// 脚本那一种。
pub const SCRIPT: &str = "script";

/// 扩展那一种。
pub const EXTENSION: &str = "extension";

/// 清单是 `manifest`、包文件夹是 `folder` 的包装上以后会多的工具。读不了的当没有。
pub fn of(manifest: &Manifest, folder: &Path) -> Vec<Tool> {
    let mut tools = scripts(&folder.join("scripts"));
    for name in manifest
        .features
        .iter()
        .flatten()
        .flat_map(|feature| &feature.tools)
    {
        tools.push(Tool {
            name: name.clone(),
            title: BTreeMap::new(),
            from: EXTENSION.to_string(),
        });
    }
    // 先照名字排、同名的脚本在前（排序是稳的），再去掉同名的后一个。
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools.dedup_by(|later, first| later.name == first.name);
    tools
}

/// `dir` 下那一层普通文件里读得成开头的脚本。
fn scripts(dir: &Path) -> Vec<Tool> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut tools = Vec::new();
    for entry in entries.flatten() {
        // `file_type` 不跟链接：链接、子目录都不是普通文件。
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        if file_name.starts_with('.') {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(head) = miyu_policy::script::read(&file_name, &text) else {
            continue;
        };
        tools.push(Tool {
            name: head.name,
            title: head
                .display_name
                .into_iter()
                .filter(|(lang, _)| lang == "zh" || lang == "en")
                .collect(),
            from: SCRIPT.to_string(),
        });
    }
    tools
}

#[cfg(test)]
mod tests;
