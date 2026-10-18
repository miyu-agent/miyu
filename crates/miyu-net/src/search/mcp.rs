//! 搜索来源里走 MCP 的几家（Exa、Parallel、Firecrawl、You.com，施工 N-2 上）：Streamable HTTP 的最小一份客户端。
//!
//! - 第一次调一家先握手（`initialize`、`notifications/initialized`），对方给了 `Mcp-Session-Id` 的记下，以后带上；会话过期
//!   （404）的重新握手一次。
//! - 回应可以是一段 JSON，也可以是 SSE（`data:` 一行一条）：取最后一条带 `result` 或 `error` 的。
//! - 只要 `tools/call` 的文字：几段文字内容接起来。`isError` 的、JSON-RPC 出错的当失败，原话交给 [`Failure::from_text`] 认
//!   是不是限额。
//!
//! 管道那一路的 MCP 客户端在 `miyu-mcp`（扩展、用户配的服务用它），这里只说 HTTP、只调一件工具，不另起依赖。

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use reqwest::Client;
use reqwest::StatusCode;
use serde_json::{Value, json};

use super::Failure;
use crate::body;

/// 我们说的版本。
const VERSION: &str = "2025-06-18";

/// 一份回应至多读几个字节。
const MOST_BYTES: usize = 4 * 1024 * 1024;

/// 记着的会话：地址 → `Mcp-Session-Id`（对方没给的记空字）。
#[derive(Default)]
pub(crate) struct Sessions {
    held: Mutex<HashMap<String, String>>,
}

impl Sessions {
    fn get(&self, url: &str) -> Option<String> {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(url)
            .cloned()
    }

    fn set(&self, url: &str, id: String) {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(url.to_string(), id);
    }

    fn drop_session(&self, url: &str) {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(url);
    }
}

/// 调 `url` 那一家的工具 `tool`，参数 `args`：交回文字。
pub(crate) async fn call(
    client: &Client,
    sessions: &Sessions,
    url: &str,
    tool: &str,
    args: Value,
    timeout: Duration,
) -> Result<String, Failure> {
    let request = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                         "params": {"name": tool, "arguments": args}});
    for _ in 0..2 {
        let session = match sessions.get(url) {
            Some(session) => session,
            None => {
                let session = open(client, url, timeout).await?;
                sessions.set(url, session.clone());
                session
            }
        };
        match post(client, url, &session, &request, timeout).await {
            Err(Failure::Gone) => sessions.drop_session(url),
            Err(failure) => return Err(failure),
            Ok(reply) => return text(&reply),
        }
    }
    Err(Failure::Failed("the session expired twice".to_string()))
}

/// 握手：交回对方给的会话编号，没给的是空字。
async fn open(client: &Client, url: &str, timeout: Duration) -> Result<String, Failure> {
    let hello = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": VERSION, "capabilities": {},
        "clientInfo": {"name": "miyu", "version": env!("CARGO_PKG_VERSION")}}});
    let response = send(client, url, "", &hello, timeout).await?;
    let session = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    reply(response).await?;
    let ready = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    // 通知不要回应：对方回 202、200 都行，出错的也不管，`tools/call` 会说。
    let _sent = send(client, url, &session, &ready, timeout).await;
    Ok(session)
}

/// 发一条要回应的，读回应。
async fn post(
    client: &Client,
    url: &str,
    session: &str,
    message: &Value,
    timeout: Duration,
) -> Result<Value, Failure> {
    let response = send(client, url, session, message, timeout).await?;
    if response.status() == StatusCode::NOT_FOUND && !session.is_empty() {
        return Err(Failure::Gone);
    }
    reply(response).await
}

/// 发出去：带上会话编号（有的话）。
async fn send(
    client: &Client,
    url: &str,
    session: &str,
    message: &Value,
    timeout: Duration,
) -> Result<reqwest::Response, Failure> {
    let mut request = client
        .post(url)
        .timeout(timeout)
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", VERSION)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(serde_json::to_vec(message).unwrap_or_default());
    if !session.is_empty() {
        request = request.header("Mcp-Session-Id", session);
    }
    request
        .send()
        .await
        .map_err(|error| Failure::Failed(error.without_url().to_string()))
}

/// 读回应：限额、别的状态码、读不懂的当失败；JSON-RPC 出错的交给 [`Failure::from_text`]。
async fn reply(response: reqwest::Response) -> Result<Value, Failure> {
    // 429 不另判：下面照原话认，「HTTP 429」就是限额。
    let status = response.status();
    let bytes = body::read_prefix(response, MOST_BYTES)
        .await
        .map_err(|_| Failure::Failed("the answer broke off".to_string()))?;
    let text = String::from_utf8_lossy(&bytes);
    if !status.is_success() {
        return Err(Failure::from_text(&format!(
            "HTTP {}: {}",
            status.as_u16(),
            head(&text)
        )));
    }
    let message = last_message(&text)
        .ok_or_else(|| Failure::Failed(format!("not a JSON-RPC answer: {}", head(&text))))?;
    if let Some(error) = message.get("error") {
        let said = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(Failure::from_text(said));
    }
    Ok(message)
}

/// 一段 JSON，或者 SSE 里最后一条带 `result`、`error` 的。
fn last_message(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        return serde_json::from_str(trimmed).ok();
    }
    trimmed
        .lines()
        .rev()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
        .find(|message| message.get("result").is_some() || message.get("error").is_some())
}

/// `tools/call` 的结果：几段文字接起来；`isError` 的当失败。
fn text(message: &Value) -> Result<String, Failure> {
    let result = message.get("result").unwrap_or(&Value::Null);
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err(Failure::from_text(&text));
    }
    Ok(text)
}

/// 记日志用的开头一截。
fn head(text: &str) -> String {
    text.chars().take(200).collect()
}

#[cfg(test)]
mod tests;
