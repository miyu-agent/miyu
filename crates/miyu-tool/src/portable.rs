//! 工具参数格式里不是各家都收的写法（施工 FX-10，`internal/blueprint/packages.md`「工具说明的参数写法」）：一件工具写了，整个请求在有的模型
//! 供应商那里被拒，不只是这一件工具用不了。
//!
//! 现在只查一种：`enum` 写在不是字符串的类型上（`integer`、`number`、`boolean`），或者 `enum` 里有不是字符串的值。Gemini 的
//! 函数声明（`FunctionDeclaration.parameters`，OpenAPI 那一套 Schema）的 `enum` 只给 `STRING`，写在整数上的整个请求回 400；接入
//! 通讯平台的一件工具写了 `{"type":"integer","enum":[1,7,30]}`，项目主人 2026-11-20 用 Antigravity 时撞上。OpenAI、Anthropic
//! 不开严格模式时都收，所以只在这一种上拦，别的写法查过官方文档、列在蓝图里，不拦。
//!
//! 只走 schema 的位置：`properties` 的每个值、`items`、`anyOf`、`oneOf`、`allOf` 的每一项、对象写的
//! `additionalProperties`。`properties` 底下名字恰好叫 `enum` 的是属性名，不当关键字。

use std::path::{Path, PathBuf};

use serde_json::Value;

/// 参数格式 `schema` 里不是各家都收的地方，一处一个位置（照 JSON Pointer 的写法，例如 `/properties/days`；根就是空的）。
pub fn unportable(schema: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk(schema, "", &mut found);
    found
}

/// 看 `at` 这一处，再往下走 schema 的位置。
fn walk(schema: &Value, at: &str, found: &mut Vec<String>) {
    let Value::Object(map) = schema else {
        return;
    };
    if let Some(Value::Array(values)) = map.get("enum") {
        let typed = map
            .get("type")
            .is_some_and(|kind| kind.as_str() != Some("string"));
        if typed || values.iter().any(|value| !value.is_string()) {
            found.push(at.to_string());
        }
    }
    if let Some(Value::Object(properties)) = map.get("properties") {
        for (name, inner) in properties {
            walk(inner, &format!("{at}/properties/{}", escaped(name)), found);
        }
    }
    for key in ["items", "additionalProperties"] {
        if let Some(inner) = map.get(key) {
            walk(inner, &format!("{at}/{key}"), found);
        }
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(Value::Array(each)) = map.get(key) {
            for (n, inner) in each.iter().enumerate() {
                walk(inner, &format!("{at}/{key}/{n}"), found);
            }
        }
    }
}

/// JSON Pointer 里一段的写法：`~` 写成 `~0`，`/` 写成 `~1`。
fn escaped(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// 包文件夹里一份工具说明不是各家都收的地方。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// 那一份相对包文件夹的路径，用 `/` 分开，例如 `data/qq/tools/qq_activity_ranking.json`。
    pub file: String,
    /// 它的 `parameters` 里那几处（[`unportable`]）。
    pub at: Vec<String>,
}

/// 查包文件夹 `folder` 里的工具说明：名字是 `tools/<名字>.json`、里面有 `parameters` 的（同 [`crate::load::spec_in`] 读的那种
/// 写法，自己带着工具说明的扩展照它放），照路径排。读不了的目录、文件、不是 JSON 的跳过：写坏了由用到它的那一头报。不跟
/// 符号链接。
pub fn scan(folder: &Path) -> Vec<Found> {
    let mut found = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(relative) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(folder.join(&relative)) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = relative.join(entry.file_name());
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() && is_spec(&path) {
                let at = std::fs::read(folder.join(&path))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    .and_then(|spec| spec.get("parameters").map(unportable))
                    .unwrap_or_default();
                if !at.is_empty() {
                    found.push(Found {
                        file: slashed(&path),
                        at,
                    });
                }
            }
        }
    }
    found.sort_by(|a, b| a.file.cmp(&b.file));
    found
}

/// `tools/<名字>.json`：上一层目录叫 `tools`，扩展名是 `json`。
fn is_spec(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "json")
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "tools")
}

/// 相对路径写成 `/` 分开的。
fn slashed(path: &Path) -> String {
    path.iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests;
