//! 读服务的回应（施工 X-32 上、PT-1 上）：整段读完、SSE 拆成一条条、回错的回应体是不是一条 JSON-RPC 错误。

use serde_json::{Value, json};

use crate::wire::LINE_LIMIT;

/// 回错的回应体 `body`（`sse` 的照事件拆，拆不出的照整段读：有的服务出错时类型还写着 SSE、正文却是 JSON）是一条 JSON-RPC 错误的：编号换成这次请求的 `id`，交回要写回去的那一行（通知没有编号，
/// 什么都不写）和记日志的一句（码和原话）。不是的没有。
pub(super) fn rpc_error(
    body: &str,
    sse: bool,
    id: Option<&Value>,
) -> Option<(Option<String>, String)> {
    let text = sse
        .then(|| events(body).into_iter().next())
        .flatten()
        .unwrap_or_else(|| body.trim().to_string());
    let mut message: Value = serde_json::from_str(&text).ok()?;
    let code = message["error"]["code"].as_i64()?;
    let said = format!(
        "{code} {}",
        message["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect::<String>()
    );
    let line = id.map(|id| {
        message["jsonrpc"] = json!("2.0");
        message["id"] = id.clone();
        if let Some(object) = message.as_object_mut() {
            object.remove("result");
        }
        message.to_string()
    });
    Some((line, said))
}

/// 读完一段回应：读断了的、超过 [`LINE_LIMIT`] 的交回原因（再长的一条客户端也收不下，不往内存里一直读）。
pub(super) async fn body(mut response: miyu_http::Response) -> Result<String, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("the answer broke off: {}", error.without_url()))?
    {
        if bytes.len() + chunk.len() > LINE_LIMIT {
            return Err(format!("the answer is longer than {LINE_LIMIT} bytes"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// SSE 的正文拆成一条条消息：空行分开一个事件，一个事件里的几行 `data:` 接起来；`\r\n` 也算换行。
pub(super) fn events(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut data: Vec<&str> = Vec::new();
    for line in body.lines().chain(std::iter::once("")) {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            if !data.is_empty() {
                out.push(data.join("\n"));
                data.clear();
            }
        } else if let Some(rest) = line.strip_prefix("data:") {
            data.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    out
}
