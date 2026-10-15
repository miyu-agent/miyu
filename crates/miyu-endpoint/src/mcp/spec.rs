//! 服务列出来的工具换成目录里的工具（施工 X-3 上，`mcp.md`「对外的样子」第 1 到 4 条）：名字 `mcp__<服务名>__<工具名>`，说明
//! 照原文截到 2048 个字符，参数照原样，访问类别照注解，给哪种会话、时限照清单。登记时查不过的那一件不登记，记一行 `WARN`。

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use miyu_config::package::McpServer;
use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::{Spec, Tool, Venues, mcp_prefix};

use super::tool::{McpTool, Texts};
use super::{Shared, TARGET};
use crate::Core;

/// 工具名最长几个字符：同目录（各家供应商对函数名的限制）。
const NAME_LIMIT: usize = 64;

/// 说明最长几个字符（同 Claude Code）。
const DESCRIPTION_LIMIT: usize = 2048;

/// 服务 `service` 的工具 `tool` 在目录里叫什么：不合 `[A-Za-z0-9_-]` 的字换成 `_`，超过 64 个字符的截到 64 个。
pub(super) fn name(service: &str, tool: &str) -> String {
    let cleaned: String = tool
        .chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                true => c,
                false => '_',
            },
        )
        .collect();
    let mut name = format!("{}{cleaned}", mcp_prefix(service));
    name.truncate(NAME_LIMIT);
    name
}

/// 说明：服务给的原文，超过 [`DESCRIPTION_LIMIT`] 个字符的截掉后面；没给的是空的。
pub(super) fn description(text: Option<&str>) -> String {
    let text = text.unwrap_or_default();
    match text.char_indices().nth(DESCRIPTION_LIMIT) {
        Some((at, _)) => text[..at].to_string(),
        None => text.to_string(),
    }
}

/// 访问类别：服务说只读的算 `read`，别的一律算 `network`（`mcp.md`「对外的样子」第 3 条）。
pub(super) fn access(tool: &miyu_mcp::Tool) -> Access {
    match tool.annotations.read_only {
        Some(true) => Access::Read,
        _ => Access::Network,
    }
}

/// 给哪种会话：照清单的 `venues`（读清单时查过了，只有这三种）。
pub(super) fn venues(server: &McpServer) -> Venues {
    let mut venues = Venues::default();
    for venue in &server.venues {
        match venue.as_str() {
            "local" => venues.local = true,
            "private" => venues.private = true,
            "group" => venues.group = true,
            _ => {}
        }
    }
    venues
}

/// 照服务 `server` 列出来的 `tools` 换掉包 `package` 里这个服务的工具：截断后撞名的后一件、目录查不过的（撞了别的包的、
/// 参数不是对象的）不登记，各记一行 `WARN mcp tool skipped`。资源目录读不出来的（安装坏了）一件都不登记。
pub(super) fn register(
    core: &Core,
    package: &str,
    server: &McpServer,
    tools: &[miyu_mcp::Tool],
    shared: &Arc<Shared>,
) {
    let service = server.name.as_str();
    let Some(texts) = Texts::load(core) else {
        tracing::error!(target: TARGET, service, "mcp texts unreadable");
        return;
    };
    let mut seen = BTreeSet::new();
    let mut built: Vec<Arc<dyn Tool>> = Vec::new();
    for tool in tools {
        let name = name(service, &tool.name);
        if !seen.insert(name.clone()) {
            tracing::warn!(target: TARGET, service, tool = tool.name.as_str(), taken_by = name.as_str(), "mcp tool skipped");
            continue;
        }
        let Ok(parameters) = serde_json::from_str::<RawJson>(&tool.input_schema.to_string()) else {
            tracing::warn!(target: TARGET, service, tool = tool.name.as_str(), problem = "parameters", "mcp tool skipped");
            continue;
        };
        built.push(Arc::new(McpTool {
            spec: Spec {
                name,
                description: description(tool.description.as_deref()),
                parameters,
                access: access(tool),
                need_load: server.need_load,
            },
            original: tool.name.clone(),
            title: tool
                .title
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(str::to_string),
            venues: venues(server),
            timeout: Duration::from_millis(server.timeout_ms),
            shared: Arc::clone(shared),
            texts: texts.clone(),
        }));
    }
    let count = built.len();
    let mut skipped = Vec::new();
    let generation = core
        .tools
        .replace(|catalog| {
            let (catalog, refused) = catalog.replacing_service(package, service, built);
            skipped = refused;
            Ok::<_, std::convert::Infallible>(catalog)
        })
        .unwrap_or_else(|never| match never {});
    tracing::info!(target: TARGET, service, tools = count - skipped.len(), generation, "mcp tools registered");
    for error in skipped {
        tracing::warn!(target: TARGET, service, tool = error.tool.as_str(), error = %error, "mcp tool skipped");
    }
}

#[cfg(test)]
mod tests;
