//! 个人设置里的 MCP 服务（施工 F-9 上，`docs/blueprint/mcp.md`「个人设置里的一张表」）：写错的照行报，`code` 同清单的
//! （`bad_mcp`、`unknown_key`）；盖住了包里同名服务的，报在那个包的清单上（`mcp_covered`，说被谁盖住，同 `script_covered`）。

use serde_json::{Value, json};

use miyu_store::human::Human;

use super::shown;
use crate::Core;
use crate::packages::sentence;

/// 个人设置的字 `text`（给人看的写法 `file`）里 MCP 服务的问题。`kind` 都是 `mcp`。
pub(super) fn personal(core: &Core, words: &Human, file: &str, text: &str) -> Vec<Value> {
    let (servers, wrong) = miyu_config::package::personal_mcp(text);
    let mut problems: Vec<Value> = wrong
        .iter()
        .map(|problem| {
            let code = problem.code.as_str();
            let message = sentence(words, code, &problem.detail);
            let mut said = json!({"kind": "mcp", "file": file, "code": code, "level": "error", "message": message.unwrap_or_else(|| problem.message.clone())});
            if let Some(line) = problem.line {
                said["line"] = json!(line);
            }
            said
        })
        .collect();
    for found in core.packages().iter() {
        let Ok(manifest) = &found.read else {
            continue;
        };
        for server in &manifest.mcp {
            if let Some(mine) = servers.iter().find(|mine| mine.name == server.name) {
                let by = format!("{file} [mcp.{}]", mine.name);
                let message = sentence(words, "mcp_covered", &by).unwrap_or_default();
                let mut said = json!({"kind": "mcp", "file": shown(core, &found.path), "code": "mcp_covered", "level": "warning", "message": message});
                if let Some(line) = server.line {
                    said["line"] = json!(line);
                }
                problems.push(said);
            }
        }
    }
    problems
}
