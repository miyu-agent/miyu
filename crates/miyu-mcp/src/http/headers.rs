//! 规范的标准请求头（施工 PT-1 上，规范 2026-07-28「Standard Request Headers」「Value Encoding」）：从一条请求里取 `Mcp-Method`、
//! `Mcp-Name`，值照规范编码。工具参数镜像的 `Mcp-Param-*` 在 `params.rs`，编码同这里。

use base64::Engine;
use serde_json::Value;

/// 照规范镜像进请求头的（施工 PT-1 上）：有方法名的带 `Mcp-Method`；`tools/call`、`prompts/get` 带 `Mcp-Name: <params.name>`，
/// `resources/read` 带 `Mcp-Name: <params.uri>`。
pub(super) fn standard_headers(message: &Value) -> Vec<(String, String)> {
    let Some(method) = message["method"].as_str() else {
        return Vec::new();
    };
    let mut headers = vec![("Mcp-Method".to_string(), header_value(method))];
    let named = match method {
        "tools/call" | "prompts/get" => message["params"]["name"].as_str(),
        "resources/read" => message["params"]["uri"].as_str(),
        _ => None,
    };
    if let Some(name) = named {
        headers.push(("Mcp-Name".to_string(), header_value(name)));
    }
    headers
}

/// 一个值写进请求头（规范「Value Encoding」）：全是可见 ASCII、空格、制表符，首尾没有空白，又不像 `=?base64?…?=` 记号的照原样；
/// 别的照 UTF-8 编成 base64，包上 `=?base64?`、`?=`。
pub(super) fn header_value(text: &str) -> String {
    let safe = text.chars().all(|c| c == '\t' || (' '..='~').contains(&c));
    let padded = text.starts_with([' ', '\t']) || text.ends_with([' ', '\t']);
    let looks_encoded = text.starts_with("=?base64?") && text.ends_with("?=");
    if safe && !padded && !looks_encoded {
        return text.to_string();
    }
    format!(
        "=?base64?{}?=",
        base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
    )
}
