//! 剧本里的资源（施工 X-41）：剧本带了资源或者模板的，本事里声明 `resources`。`resources/list`、`resources/templates/list`
//! 照剧本的 `page` 翻页；`resources/read` 照地址答：`file:///notes.txt` 一段字，`file:///dot.png` 一张图（字节是假的，
//! 适配器原样交），`file:///blob.bin` 一段别的二进制，`file:///logs/<哪天>.txt`（模板）一段字，别的回「没有这个资源」（`-32002`）。

use serde_json::{Value, json};

use super::{Script, complete};
use crate::wire;

/// 剧本里的几份资源：一份字、一张图、一段二进制，外加一项没有地址的（写法不对，客户端不收）。多写的 `_meta` 客户端丢掉。
pub fn resources() -> Vec<Value> {
    vec![
        json!({"uri": "file:///notes.txt", "name": "notes", "title": "Notes", "description": "Things to remember.", "mimeType": "text/plain", "size": 18, "_meta": {"x": 1}}),
        json!({"name": "no uri"}),
        json!({"uri": "file:///dot.png", "name": "dot", "mimeType": "image/png"}),
        json!({"uri": "file:///blob.bin", "name": "blob", "mimeType": "application/octet-stream"}),
    ]
}

/// 剧本里的一个模板。
pub fn templates() -> Vec<Value> {
    vec![json!({
        "uriTemplate": "file:///logs/{day}.txt",
        "name": "day log",
        "description": "The log of one day.",
        "mimeType": "text/plain",
        "annotations": {"audience": ["user"]},
    })]
}

/// 本事里声明不声明 `resources`。
pub(super) fn offered(script: &Script) -> bool {
    !script.resources.is_empty() || !script.resource_templates.is_empty()
}

/// `resources/list`（`key` 是 `resources`）、`resources/templates/list`（`resourceTemplates`）的一页：同工具翻页。
pub(super) fn page(script: &Script, id: &Value, params: &Value, key: &str) -> Vec<u8> {
    let items = match key {
        "resources" => &script.resources,
        _ => &script.resource_templates,
    };
    let start = params
        .get("cursor")
        .and_then(Value::as_str)
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or(0);
    let size = match script.page {
        0 => items.len(),
        page => page,
    };
    let end = (start + size).min(items.len());
    let mut result = json!({ key: items.get(start..end).unwrap_or_default() });
    if end < items.len() {
        result["nextCursor"] = json!(end.to_string());
    }
    complete(script, id, result)
}

/// `resources/read`：照地址答。
pub(super) fn read(script: &Script, id: &Value, params: &Value) -> Vec<u8> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let content = match uri {
        "file:///notes.txt" => {
            json!({"uri": uri, "mimeType": "text/plain", "text": "Remember the milk."})
        }
        "file:///dot.png" => json!({"uri": uri, "mimeType": "image/png", "blob": "aGk="}),
        "file:///blob.bin" => {
            json!({"uri": uri, "mimeType": "application/octet-stream", "blob": "AAECAw=="})
        }
        _ => match uri
            .strip_prefix("file:///logs/")
            .and_then(|rest| rest.strip_suffix(".txt"))
        {
            Some(day) => {
                json!({"uri": uri, "mimeType": "text/plain", "text": format!("Log of {day}.")})
            }
            None => return wire::answer(id, Err((-32002, "Resource not found"))),
        },
    };
    complete(script, id, json!({"contents": [content]}))
}
