//! 个人设置里的 MCP 服务（施工 F-9 上，`docs/blueprint/mcp.md`「个人设置里的一张表」）：写错的照行报，`code` 同清单的
//! （`bad_mcp`、`unknown_key`）；盖住了包里同名服务的，报在那个包的清单上（`mcp_covered`，说被谁盖住，同 `script_covered`）。
//! 列出来却没登记的工具（施工 X-37，[`skipped`]）：个人设置里的、包带的服务，这一次列完有没登记的，各报一条 `mcp_tool_skipped`。

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_store::scripts::HOME_PACKAGE;

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
    for found in core.system_packages().iter() {
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

/// 列出来却没登记的工具（施工 X-37）：一个服务一条警告，`detail` 写服务名和每一件的原名、为什么。个人设置里的报在个人设置上，
/// 包带的报在那个包的清单上。只看这一次列出来的：没起过、照缓存先登记的没有。
pub(super) fn skipped(core: &Core, words: &Human) -> Vec<Value> {
    let warn = |file: String, service: &str, package: &str| {
        let skipped = core.mcp.skipped(package, service);
        if skipped.is_empty() {
            return None;
        }
        // 没写名字的只说为什么；提示词写成 `/<原名>`（施工 X-39）。
        let tools: Vec<String> = skipped
            .iter()
            .map(|skipped| match (skipped.name.as_str(), skipped.prompt) {
                ("", _) => format!("({})", skipped.why),
                (name, true) => format!("/{name} ({})", skipped.why),
                (name, false) => format!("{name} ({})", skipped.why),
            })
            .collect();
        let detail = format!("{service}: {}", tools.join(", "));
        let message = sentence(words, "mcp_tool_skipped", &detail).unwrap_or_default();
        Some(
            json!({"kind": "mcp", "file": file, "code": "mcp_tool_skipped", "level": "warning", "message": message}),
        )
    };
    let mine = core.config().personal.shown.clone();
    let mut problems: Vec<Value> = core
        .home_mcp
        .now()
        .iter()
        .filter_map(|server| warn(mine.clone(), &server.name, HOME_PACKAGE))
        .collect();
    for found in core.system_packages().iter() {
        let Ok(manifest) = &found.read else {
            continue;
        };
        problems.extend(
            manifest
                .mcp
                .iter()
                .filter_map(|server| warn(shown(core, &found.path), &server.name, &found.id)),
        );
    }
    problems
}
