//! 重场景里的 MCP 服务（施工 LW-30）：量尺自己扮。命令型的那一个是量尺带参数 [`ARG`] 起来，经标准输入输出说旧时代
//! （`server/discover` 回「没有这个方法」，等 `initialize`）；网址的两个由 `heavy/web.rs` 在同一个进程里答，说新时代。两种都照
//! [`answer`] 答，答法照核心仓库测试用的服务（`crates/miyu-endpoint/src/bin/miyu-test-mcp.rs`）。
//!
//! 三件工具：`echo`（标了只读）、`add`（带结构化结果）、`note`（回一段字和一条资源链接）；三份资源、一个模板；两个提示词。

use std::io::{BufRead, Write};

use serde_json::{Value, json};

/// 量尺照这个参数起来就当命令型的 MCP 服务（[`serve`]）。
pub const ARG: &str = "heavy-mcp";

/// 声明的本事：工具、提示词、资源。
fn capabilities() -> Value {
    json!({"tools": {"listChanged": true}, "prompts": {"listChanged": true}, "resources": {}})
}

/// 列出来的三件工具。
fn tools() -> Value {
    json!([
        {"name": "echo", "title": "Echo back", "description": "Echo the text.",
         "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}},
         "annotations": {"readOnlyHint": true}},
        {"name": "add", "description": "Add two numbers.",
         "inputSchema": {"type": "object", "properties": {"a": {"type": "number"}, "b": {"type": "number"}},
                         "required": ["a", "b"]}},
        {"name": "note", "description": "Write a note.",
         "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}},
    ])
}

/// 调一件工具的结果。
fn called(params: &Value) -> Value {
    let arguments = &params["arguments"];
    match params["name"].as_str() {
        Some("add") => {
            let sum =
                arguments["a"].as_f64().unwrap_or(0.0) + arguments["b"].as_f64().unwrap_or(0.0);
            json!({"content": [{"type": "text", "text": sum.to_string()}], "structuredContent": {"sum": sum}})
        }
        Some("note") => json!({"content": [{"type": "text", "text": "noted"},
            {"type": "resource_link", "uri": "file:///notes.txt", "name": "notes"}]}),
        _ => {
            let text = arguments["text"].as_str().unwrap_or_default();
            json!({"content": [{"type": "text", "text": text}]})
        }
    }
}

/// 答一条 `message`：带编号的请求交回一条回应，通知（没有编号）交回 `None`。`legacy` 的是旧时代：`server/discover` 回
/// 「没有这个方法」，结果里不带 `resultType`；新时代的结果都带 `"resultType": "complete"`。
pub fn answer(message: &Value, legacy: bool) -> Option<Value> {
    let id = message.get("id")?.clone();
    let params = &message["params"];
    let result = match message["method"].as_str().unwrap_or_default() {
        "server/discover" if legacy => None,
        "server/discover" => Some(
            json!({"supportedVersions": ["2026-07-28"], "capabilities": capabilities(),
            "instructions": "Use echo to test.", "serverInfo": {"name": "perf-heavy", "version": "0"}}),
        ),
        "initialize" => Some(
            json!({"protocolVersion": "2025-11-25", "capabilities": capabilities(),
            "instructions": "Use echo to test.", "serverInfo": {"name": "perf-heavy", "version": "0"}}),
        ),
        "tools/list" => Some(json!({"tools": tools()})),
        "tools/call" => Some(called(params)),
        "prompts/list" => Some(json!({"prompts": [
            {"name": "review", "description": "Review code.", "arguments": [{"name": "code", "required": true}]},
            {"name": "hello", "description": "Say hello."}]})),
        "prompts/get" => Some(
            json!({"messages": [{"role": "user", "content": {"type": "text",
            "text": format!("Please review: {}", params["arguments"]["code"].as_str().unwrap_or_default())}}]}),
        ),
        "resources/list" => Some(json!({"resources": [
            {"uri": "file:///notes.txt", "name": "notes", "mimeType": "text/plain"},
            {"uri": "file:///readme.md", "name": "readme", "mimeType": "text/markdown"},
            {"uri": "file:///data.json", "name": "data", "mimeType": "application/json"}]})),
        "resources/templates/list" => Some(json!({"resourceTemplates": [
            {"uriTemplate": "file:///logs/{day}.txt", "name": "day log", "mimeType": "text/plain"}]})),
        "resources/read" => {
            let uri = params["uri"].as_str().unwrap_or_default();
            Some(json!({"contents": [{"uri": uri, "mimeType": "text/plain",
                "text": format!("Contents of {uri}: line one.\nline two.")}]}))
        }
        "ping" => Some(json!({})),
        _ => None,
    };
    Some(match result {
        Some(mut result) => {
            if !legacy {
                result["resultType"] = json!("complete");
            }
            json!({"jsonrpc": "2.0", "id": id, "result": result})
        }
        None => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Method not found"}})
        }
    })
}

/// 当命令型的 MCP 服务：一行一条，照 [`answer`] 答旧时代；标准输入读到头就退出。
pub fn serve() -> std::process::ExitCode {
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(reply) = answer(&message, true)
            && writeln!(out, "{reply}").and_then(|()| out.flush()).is_err()
        {
            break;
        }
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_era_refuses_discovery_and_answers_initialize() {
        let discover = json!({"jsonrpc": "2.0", "id": 1, "method": "server/discover"});
        assert_eq!(
            answer(&discover, true).expect("有编号")["error"]["code"],
            -32601
        );
        let init = json!({"jsonrpc": "2.0", "id": 2, "method": "initialize"});
        let reply = answer(&init, true).expect("有编号");
        assert_eq!(reply["result"]["protocolVersion"], "2025-11-25");
        assert!(reply["result"].get("resultType").is_none());
    }

    #[test]
    fn the_new_era_marks_results_complete_and_notifications_get_nothing() {
        let call = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "add", "arguments": {"a": 2, "b": 3}}});
        let reply = answer(&call, false).expect("有编号");
        assert_eq!(reply["result"]["resultType"], "complete");
        assert_eq!(reply["result"]["structuredContent"]["sum"], 5.0);
        assert!(
            answer(
                &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
                false
            )
            .is_none()
        );
    }
}
