//! 服务列出来的工具换成目录里的工具（施工 X-3 上，`mcp.md`「对外的样子」第 1 到 4 条）：名字 `mcp__<服务名>__<工具名>`，说明
//! 照原文截到 2048 个字符，参数照原样，访问类别照注解，给哪种会话、时限照清单。登记时查不过的那一件不登记，记一行 `WARN`。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use miyu_config::package::McpServer;
use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::{Problem, Spec, Tool, Venues, mcp_prefix};

use super::tool::{McpTool, Texts};
use super::{Shared, Skipped, TARGET};
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
/// 参数不是对象的）不登记，各记一行 `WARN mcp tool skipped`，交回这几件（施工 X-37：列完的记进看管那一格，`mcp.list`、软件包
/// 列表、`miyu check` 照它说；照缓存登记的不记）。资源目录读不出来的（安装坏了）一件都不登记。
pub(super) fn register(
    core: &Core,
    package: &str,
    server: &McpServer,
    tools: &[miyu_mcp::Tool],
    shared: &Arc<Shared>,
) -> Vec<Skipped> {
    let service = server.name.as_str();
    let Some(texts) = Texts::load(core) else {
        tracing::error!(target: TARGET, service, "mcp texts unreadable");
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut built: Vec<Arc<dyn Tool>> = Vec::new();
    let mut skipped = Vec::new();
    // 目录里的名字到服务那边的原名：目录拒收的照原名说。
    let mut originals = BTreeMap::new();
    for tool in tools {
        let name = name(service, &tool.name);
        if !seen.insert(name.clone()) {
            tracing::warn!(target: TARGET, service, tool = tool.name.as_str(), taken_by = name.as_str(), "mcp tool skipped");
            skipped.push(Skipped::tool(tool.name.clone(), "name_taken"));
            continue;
        }
        let Ok(parameters) = serde_json::from_str::<RawJson>(&tool.input_schema.to_string()) else {
            tracing::warn!(target: TARGET, service, tool = tool.name.as_str(), problem = "parameters", "mcp tool skipped");
            skipped.push(Skipped::tool(tool.name.clone(), "parameters"));
            continue;
        };
        originals.insert(name.clone(), tool.name.clone());
        built.push(Arc::new(McpTool {
            spec: Spec {
                name,
                description: description(tool.description.as_deref()),
                parameters,
                access: access(tool),
                need_load: server.need_load,
                clicall: server.clicall,
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
    let mut refused = Vec::new();
    let generation = core
        .tools
        .replace(|catalog| {
            let (catalog, errors) = catalog.replacing_service(package, service, built);
            refused = errors;
            Ok::<_, std::convert::Infallible>(catalog)
        })
        .unwrap_or_else(|never| match never {});
    tracing::info!(target: TARGET, service, tools = count - refused.len(), generation, "mcp tools registered");
    for error in refused {
        tracing::warn!(target: TARGET, service, tool = error.tool.as_str(), error = %error, "mcp tool skipped");
        let why = match error.problem {
            Problem::Duplicate => "name_taken",
            Problem::Name => "name",
            Problem::Parameters => "parameters",
        };
        let tool = originals.remove(&error.tool).unwrap_or(error.tool);
        skipped.push(Skipped::tool(tool, why));
    }
    skipped
}

#[cfg(test)]
mod tests;
