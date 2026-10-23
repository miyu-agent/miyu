//! 给她看的「脚本调工具」（施工 M-4，设计 `39-脚本调工具.md` 第九节）：工具面上有脚本里能调的工具时，`shell` 的说明末尾接上
//! 怎么用的那一段（字归 `shell`，`Tool::clicall_section`），system 里带上什么时候用的那一句（`core/clicall.txt`）。能调的照内核的口径：
//! `clicall` 开着、访问类别准的（`Access::scriptable`），而且工具面上有 `shell`：通道只开在它跑的命令里。
//!
//! 施工 M-6（项目主人 2026-10-23 定）：「脚本调工具」是基础系统的一个功能，预设能关（[`on`]）；关着的会话每件工具都当不能在脚本里
//! 调，于是不开通道、`shell` 的说明和 system 都回到没有它时的样子。

use std::collections::BTreeSet;

use miyu_policy::ToolEntry;
use miyu_policy::preset::PresetFile;
use miyu_tool::Catalog;
use miyu_tool::{BASESYSTEM, SHELL};

/// 「脚本调工具」这个功能的编号（施工 M-6）：基础系统里的，没有工具。
pub(crate) const FEATURE: &str = "clicall";

/// 预设 `preset` 开着「脚本调工具」没有：出厂的常规开发（整个基础系统）、全部功能开着，极简开发关着。没有预设的照开着。
pub(crate) fn on(preset: Option<&PresetFile>) -> bool {
    preset.is_none_or(|file| file.opens_in(FEATURE, BASESYSTEM))
}

/// MCP 工具名的开头：`mcp__<服务>__<工具>`。
const MCP: &str = "mcp__";

/// 工具面 `face` 上脚本里能调的那几件；没有 `shell` 的一件都没有。
fn scriptable(face: &[ToolEntry]) -> impl Iterator<Item = &ToolEntry> {
    let shell = face.iter().any(|entry| entry.name == SHELL);
    face.iter()
        .filter(move |entry| shell && entry.clicall && entry.access.scriptable())
}

/// 这个工具面要不要 system 里那一句。
pub(crate) fn tells(face: &[ToolEntry]) -> bool {
    scriptable(face).next().is_some()
}

/// 有能调的，照 `catalog` 给 `shell` 的说明接上那一段：名单照名字排、一个 MCP 服务的并成一项；有结构化结果的列出样子。
pub(crate) fn describe(catalog: &Catalog, face: &mut [ToolEntry]) {
    let names: BTreeSet<String> = scriptable(face).map(|entry| listed(&entry.name)).collect();
    if names.is_empty() {
        return;
    }
    let shapes: Vec<String> = scriptable(face)
        .filter_map(|entry| {
            let result = catalog.get(&entry.name)?.spec().result.as_ref()?;
            Some(format!("{} {}", entry.name, result.trim()))
        })
        .collect();
    let tools: Vec<&str> = names.iter().map(String::as_str).collect();
    let section = catalog
        .get(SHELL)
        .and_then(|shell| shell.clicall_section(&tools.join(", "), &shapes.join("; ")));
    if let (Some(section), Some(shell)) =
        (section, face.iter_mut().find(|entry| entry.name == SHELL))
    {
        shell.description = format!("{}\n\n{section}", shell.description.trim_end());
    }
}

/// 名单里写成什么：MCP 的照服务并成 `mcp__<服务>__*`，别的照名字。
fn listed(name: &str) -> String {
    name.strip_prefix(MCP)
        .and_then(|rest| rest.split_once("__"))
        .map_or_else(
            || name.to_string(),
            |(server, _)| format!("{MCP}{server}__*"),
        )
}

#[cfg(test)]
mod tests;
