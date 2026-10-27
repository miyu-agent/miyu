//! 服务的资源（施工 X-41，`docs/blueprint/mcp.md`「怎么走」第七条）：声明了 `resources` 的服务跟着它的工具多登记两件
//! `mcp__<服务名>__list_resources`、`mcp__<服务名>__read_resource`，同一个包、同一个前缀，所以预设、场所、先加载、撤下都照
//! 它的工具。列的一次一页，回一行 JSON（结构照 MCP，只留规范里那几格，模板只在第一页）；读的照工具结果的规矩换块（图照工具交的图
//! 收，别的二进制写一行）。服务自己有同名工具的，服务的赢。

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_config::package::McpServer;
use miyu_kernel::tool::Access;
use miyu_mcp::{Called, Client, Content, Failed};
use miyu_tool::{Spec, Tool};

use super::spec::{name, venues};
use super::tool::{McpTool, Texts, What};
use super::{Shared, TARGET};
use crate::Core;

/// 列资源那一件的工具名（去掉前缀）。
const LIST: &str = "list_resources";

/// 读资源那一件的工具名（去掉前缀）。
const READ: &str = "read_resource";

/// 服务 `server` 声明了资源时多登记的两件：说明读不出来的（安装坏了）没有；`taken` 里已经有的名字（服务自己的工具）不登记。
pub(super) fn tools(
    core: &Core,
    server: &McpServer,
    shared: &Arc<Shared>,
    texts: &Texts,
    taken: &BTreeSet<String>,
) -> Vec<Arc<dyn Tool>> {
    let service = server.name.as_str();
    let (list, read) = match core.resources.mcp_resource_tools() {
        Ok(specs) => specs,
        Err(error) => {
            tracing::error!(target: TARGET, service, error = %error, "mcp resource tools unreadable");
            return Vec::new();
        }
    };
    [
        (LIST, list, What::ListResources),
        (READ, read, What::ReadResource),
    ]
    .into_iter()
    .filter_map(|(tool, text, what)| {
        let name = name(service, tool);
        if taken.contains(&name) {
            tracing::warn!(target: TARGET, service, tool, "mcp resource tool skipped");
            return None;
        }
        let built: Arc<dyn Tool> = Arc::new(McpTool {
            spec: Spec {
                name,
                description: text.description,
                parameters: text.parameters,
                access: Access::Read,
                clicall: server.clicall,
                result: None,
                plugin: None,
            },
            what,
            title: None,
            venues: venues(server),
            timeout: Duration::from_millis(server.timeout_ms),
            shared: Arc::clone(shared),
            texts: texts.clone(),
        });
        Some(built)
    })
    .collect()
}

/// 列一页：参数的 `cursor` 是上一页交的，没写的是第一页，第一页另列地址模板（服务没实现模板的照没有）。
pub(super) async fn list(client: &Client, arguments: &Value) -> Result<Called, Failed> {
    let cursor = arguments
        .get("cursor")
        .and_then(Value::as_str)
        .filter(|cursor| !cursor.is_empty());
    let resources = client.resources(cursor).await?;
    let mut page = json!({"resources": resources.items});
    if cursor.is_none() {
        match client.resource_templates(None).await {
            Ok(templates) if !templates.items.is_empty() => {
                page["resourceTemplates"] = json!(templates.items);
            }
            Ok(_) | Err(Failed::Rpc { .. }) => {}
            Err(error) => return Err(error),
        }
    }
    if let Some(next) = resources.next {
        page["nextCursor"] = json!(next);
    }
    Ok(Called {
        content: vec![Content::Text(page.to_string())],
        structured: Some(page),
        is_error: false,
    })
}

/// 读一份：参数的 `uri`。图的二进制换成图（照工具交的图收、受同样的上限），别的照资源交给换块的那一步。
pub(super) async fn read(client: &Client, arguments: &Value) -> Result<Called, Failed> {
    let uri = arguments
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let content = client
        .read_resource(uri)
        .await?
        .into_iter()
        .map(picture)
        .collect();
    Ok(Called {
        content,
        structured: None,
        is_error: false,
    })
}

/// 类型是图的二进制资源换成图；别的原样。
fn picture(content: Content) -> Content {
    match content {
        Content::Resource {
            mime: Some(mime),
            blob: Some(data),
            ..
        } if mime.starts_with("image/") => Content::Image { data, mime },
        other => other,
    }
}

#[cfg(test)]
mod tests;
