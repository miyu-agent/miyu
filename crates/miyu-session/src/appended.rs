//! 扩展放在 system 尾部的那几段（施工 X-20，`docs/designs/34-表情包.md` 第五节第 5 条）：照这一刻的工具面挑出有工具在面上的
//! 包，它说过的照它说的，没说过的照快照里钉的；工具不在面上的包不带。
//!
//! MCP 服务交的说明也走这里（施工 X-39，`mcp.md`「怎么走」第三条第 5 条）：一个服务一段，键是 [`mcp_key`]，工具面上有这个服务的
//! 工具才带。appended 在 system 里，变了整个前缀的缓存都断，所以 MCP 的那几段只跟工具面一起变：工具面变了的那一轮照这一刻的
//! 说明换，没变的照快照里钉的（服务升级换了说明原文也不读）。扩展放的照 X-20 原来的规矩，下一轮换上。

use std::collections::BTreeMap;

use miyu_policy::ToolEntry;
use miyu_tool::{Catalog, mcp_service};

/// MCP 服务那一段的键的开头：包的编号里没有 `:`，撞不上扩展放的。
const MCP: &str = "mcp:";

/// 服务 `service` 的说明那一段的键（施工 X-39）：`mcp:<服务名>`。端点照它交，这里照它认。
pub fn mcp_key(service: &str) -> String {
    format!("{MCP}{service}")
}

/// 键 `key` 是扩展放的那一段（不是 MCP 服务的说明）：只有这种说了变了才算这一轮要重拼（MCP 的跟着工具面变）。
pub(crate) fn from_extension(key: &str) -> bool {
    !key.starts_with(MCP)
}

/// 这一轮要放的那几段：`pinned` 是快照里钉的，`asked` 是扩展这一刻说的、MCP 服务这一刻的说明（经端口问到的，`None` 是去掉
/// 了）。`fresh` 是这一轮的工具面和钉下的不一样（造会话时是）：MCP 的那几段只在这时照 `asked` 换。
pub(crate) fn resolve(
    catalog: &Catalog,
    face: &[ToolEntry],
    (pinned, asked): (&BTreeMap<String, String>, &BTreeMap<String, Option<String>>),
    fresh: bool,
) -> BTreeMap<String, String> {
    face.iter()
        .flat_map(|tool| {
            let package = catalog.package_of(&tool.name).map(str::to_string);
            let service = mcp_service(&tool.name).map(mcp_key);
            package.into_iter().chain(service)
        })
        .filter_map(|key| {
            let text = match (key.starts_with(MCP), fresh) {
                // MCP 的：工具面变了照这一刻的（这时没交说明的就没有），没变的照钉下的。
                (true, true) => asked.get(&key).cloned().flatten(),
                (true, false) => pinned.get(&key).cloned(),
                // 扩展放的：说过的照说的，没说过的照钉下的（X-20）。
                (false, _) => match asked.get(&key) {
                    Some(said) => said.clone(),
                    None => pinned.get(&key).cloned(),
                },
            };
            Some((key, text?))
        })
        .collect()
}

#[cfg(test)]
mod tests;
