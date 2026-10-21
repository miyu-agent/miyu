//! MCP 服务的资源那两件工具的说明和参数格式（施工 X-41，`docs/blueprint/mcp.md`「怎么走」第七条）：`core/mcp/list_resources.json`、
//! `core/mcp/read_resource.json`，写法同内置工具的说明文件。不进快照：同 MCP 服务自己的工具，随登记进目录。

use std::io;

use miyu_kernel::raw::RawJson;
use serde::Deserialize;

use super::{ResourceRoot, SourceError};

/// 一件工具的说明和参数格式。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolText {
    /// 说明。
    pub description: String,
    /// 参数格式，原样。
    pub parameters: RawJson,
}

impl ResourceRoot {
    /// 列资源、读资源两件工具的说明和参数格式，照这个先后。
    ///
    /// # Errors
    ///
    /// 读不了哪一份；不是 `{description, parameters}` 的样子。
    pub fn mcp_resource_tools(&self) -> Result<(ToolText, ToolText), SourceError> {
        Ok((
            self.tool_text("list_resources.json")?,
            self.tool_text("read_resource.json")?,
        ))
    }

    /// `core/mcp/<file>` 读成一件工具的说明和参数格式。
    fn tool_text(&self, file: &str) -> Result<ToolText, SourceError> {
        let parts = ["core", "mcp", file];
        serde_json::from_str(&self.read(&parts)?).map_err(|error| SourceError::Read {
            path: self.path.join(parts.join("/")),
            error: io::Error::new(io::ErrorKind::InvalidData, error.to_string()),
        })
    }
}
