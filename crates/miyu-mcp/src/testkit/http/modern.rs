//! 演新时代的假服务怎么答（施工 X-32 补、PT-1 上、PT-1 中）：照规范 2026-07-28 查请求头、认版本，答 `server/discover`、列工具、
//! 调工具（开了 [`super::HttpServer::header_tools`] 的多两件带 `x-mcp-header` 的）。

use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use super::{Received, State};

/// 新时代的答法（施工 X-32 补，施工 PT-1 上照规范 2026-07-28 的「Server Validation」查全）：`_meta` 里的版本要和
/// `MCP-Protocol-Version` 头一样，`Mcp-Method` 要和方法名一样，`tools/call`、`resources/read`、`prompts/get` 的 `Mcp-Name`（解开
/// `=?base64?…?=` 以后）要和名字、网址一样，缺了、对不上的回 `400` 和 `-32020`；版本不认的回 `400` 和 `-32022`、列出认的几版；
/// 认 `server/discover`、`tools/list`、`tools/call`，结果带 `resultType`；别的方法（`initialize` 也是）回 `404` 和「没有这个方法」。
pub(super) fn reply(state: &State, request: &Received, id: &Value) -> (u16, String, String) {
    let message = &request.body;
    let asked = message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str();
    let refuse = |code: i64, said: &str, data: Option<Value>| {
        let mut error = json!({"code": code, "message": said});
        if let Some(data) = data {
            error["data"] = data;
        }
        let refused = json!({"jsonrpc": "2.0", "id": id, "error": error});
        (400, String::new(), refused.to_string())
    };
    if asked.is_some() && request.header("mcp-protocol-version") != asked {
        return refuse(
            -32020,
            "Mcp-Protocol-Version header is required for requests carrying \"io.modelcontextprotocol/protocolVersion\"",
            None,
        );
    }
    let method = message["method"].as_str().unwrap_or_default();
    if asked.is_some() && request.header("mcp-method") != Some(method) {
        return refuse(-32020, "Header mismatch: Mcp-Method", None);
    }
    let named = match method {
        "tools/call" | "prompts/get" => message["params"]["name"].as_str(),
        "resources/read" => message["params"]["uri"].as_str(),
        _ => None,
    };
    if asked.is_some()
        && let Some(named) = named
        && request.header("mcp-name").map(decoded).as_deref() != Some(named)
    {
        return refuse(-32020, "Header mismatch: Mcp-Name", None);
    }
    let supported = state
        .supported
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(asked) = asked
        && !supported.iter().any(|version| version == asked)
    {
        return refuse(
            -32022,
            "Unsupported protocol version",
            Some(json!({"supported": supported, "requested": asked})),
        );
    }
    let result = match message["method"].as_str() {
        Some("server/discover") => {
            json!({"resultType": "complete", "supportedVersions": supported,
            "capabilities": {"tools": {}}, "_meta": {"io.modelcontextprotocol/serverInfo": {"name": "fake", "version": "1"}}})
        }
        Some("tools/list") => {
            let mut tools = vec![json!({"name": "echo", "description": "Say it back.",
                "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}})];
            if state.header_tools.load(Ordering::Relaxed) {
                tools.push(json!({"name": "region", "description": "Say the region.",
                    "inputSchema": {"type": "object", "properties": {"region": {"type": "string", "x-mcp-header": "Region"}}}}));
                tools.push(json!({"name": "broken", "description": "A misplaced mark.",
                    "inputSchema": {"type": "object", "properties": {"zones": {"type": "array",
                    "items": {"type": "string", "x-mcp-header": "Zone"}}}}}));
            }
            json!({"resultType": "complete", "tools": tools})
        }
        Some("tools/call") if message["params"]["name"] == "region" => {
            let region = message["params"]["arguments"]["region"].as_str();
            if region.is_some()
                && request.header("mcp-param-region").map(decoded).as_deref() != region
            {
                return refuse(-32020, "Header mismatch: Mcp-Param-Region", None);
            }
            json!({"resultType": "complete",
            "content": [{"type": "text", "text": region.unwrap_or_default()}]})
        }
        Some("tools/call") if message["params"]["name"] == "echo" => {
            json!({"resultType": "complete",
            "content": [{"type": "text", "text": message["params"]["arguments"]["text"]}]})
        }
        _ => {
            let missing = json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "method not found"}});
            return (404, String::new(), missing.to_string());
        }
    };
    let answer = json!({"jsonrpc": "2.0", "id": id, "result": result});
    let text = if state.sse {
        format!("event: message\ndata: {answer}\n\n")
    } else {
        answer.to_string()
    };
    (200, String::new(), text)
}

/// 请求头的值照规范解开：`=?base64?…?=` 的解 base64，别的原样。
fn decoded(value: &str) -> String {
    use base64::Engine;
    value
        .strip_prefix("=?base64?")
        .and_then(|rest| rest.strip_suffix("?="))
        .and_then(|inner| base64::engine::general_purpose::STANDARD.decode(inner).ok())
        .map_or_else(
            || value.to_string(),
            |bytes| String::from_utf8_lossy(&bytes).into_owned(),
        )
}

/// `steps` 分两次写（施工 PT-1 下）：SSE 回应头和一条进度（带了进度令牌的才有）先写出去，隔一秒再写结果，写完关连接（没有
/// `Content-Length`，读到关为止）。
pub(super) async fn steps(mut stream: TcpStream, request: &Received) {
    let id = &request.body["id"];
    let head = "HTTP/1.1 200 X\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
    let mut first = head.to_string();
    if let Some(token) = request.body["params"]["_meta"].get("progressToken") {
        let progress = json!({"jsonrpc": "2.0", "method": "notifications/progress",
            "params": {"progressToken": token, "progress": 1, "total": 2, "message": "half"}});
        first.push_str(&format!("event: message\ndata: {progress}\n\n"));
    }
    if stream.write_all(first.as_bytes()).await.is_err() || stream.flush().await.is_err() {
        return;
    }
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let answer = json!({"jsonrpc": "2.0", "id": id, "result": {"resultType": "complete",
        "content": [{"type": "text", "text": "stepped"}]}});
    let written = stream
        .write_all(format!("event: message\ndata: {answer}\n\n").as_bytes())
        .await;
    if written.is_err() || stream.shutdown().await.is_err() {
        // 客户端先走了：这一次不用再答。
    }
}
