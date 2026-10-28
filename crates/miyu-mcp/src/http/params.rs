//! 工具参数镜像成请求头（施工 PT-1 中，规范 2026-07-28「Custom Headers from Tool Parameters」）：服务在工具的 `inputSchema` 里给某个
//! 参数标 `x-mcp-header: "<名>"`，Streamable HTTP 的客户端调这件工具时必须把那个参数的值放进 `Mcp-Param-<名>` 请求头。
//!
//! - 列工具时照规范查每件工具的标记：名字不空、只用 HTTP 头名许的字（`tchar`），同一份参数格式里不分大小写不重名，只标在
//!   `string`、`integer`、`boolean` 的参数上，而且这个参数从根上只经一串 `properties` 就到得了（不经 `items`、`anyOf` 这些）。
//!   有一处不合的整件工具不收（规范要求从列表里剔掉），记一条警告说哪件、为什么。
//! - 调工具时照记下的路径取值：字照原样，整数写成十进制，真假写成 `true`、`false`；那里没有值、是 `null` 的不带这个头。值的编码
//!   同 `Mcp-Name`（[`super::header_value`]）。
//! - stdio 的服务规范许不理这个标记，只在网址的服务这里做。

use std::sync::PoisonError;

use serde_json::{Map, Value};

/// 标记的键名。
const MARK: &str = "x-mcp-header";

/// 格式里写实例数据的几格：里面的键不是格式，不查标记。
const DATA: [&str; 4] = ["const", "default", "enum", "examples"];

/// 一个镜像成请求头的参数：头名里 `Mcp-Param-` 后面那段，和参数在 `arguments` 里的路径（一串属性名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Param {
    pub(super) name: String,
    pub(super) path: Vec<String>,
}

/// 查工具的参数格式 `schema` 里的标记：合规矩的交回镜像哪几个参数（没标的是空的），不合的交回为什么（英文）。
pub(super) fn of(schema: &Value) -> Result<Vec<Param>, String> {
    let mut found = Vec::new();
    walk(schema, Some(&mut Vec::new()), &mut found)?;
    let mut seen: Vec<String> = Vec::new();
    for param in &found {
        let lower = param.name.to_ascii_lowercase();
        if seen.contains(&lower) {
            return Err(format!("{MARK} {:?} is used twice", param.name));
        }
        seen.push(lower);
    }
    Ok(found)
}

/// 走一遍格式的一个节点 `node`：`path` 是从根上只经 `properties` 到这里的路径，经过别的关键字的是空的。
fn walk(
    node: &Value,
    path: Option<&mut Vec<String>>,
    found: &mut Vec<Param>,
) -> Result<(), String> {
    match node {
        Value::Object(map) => {
            let mut path = path;
            if let Some(mark) = map.get(MARK) {
                found.push(marked(map, mark, path.as_deref())?);
            }
            for (key, child) in map {
                // 标记本身、写实例数据的几格（默认值、例子这些里面的键不是格式）不往下走。
                if key == MARK || DATA.contains(&key.as_str()) {
                    continue;
                }
                match (key.as_str(), child) {
                    // `properties` 的键是参数名，不是格式：叫 `x-mcp-header` 的参数不当标记。
                    ("properties", Value::Object(properties)) => {
                        for (name, sub) in properties {
                            match path.as_deref_mut() {
                                Some(path) => {
                                    path.push(name.clone());
                                    let walked = walk(sub, Some(path), found);
                                    path.pop();
                                    walked?;
                                }
                                None => walk(sub, None, found)?,
                            }
                        }
                    }
                    _ => walk(child, None, found)?,
                }
            }
            Ok(())
        }
        Value::Array(items) => items.iter().try_for_each(|item| walk(item, None, found)),
        _ => Ok(()),
    }
}

/// 节点 `map` 上的标记 `mark`：照规范查名字、种类、位置（`path` 没有的、是根本身的不算到得了的参数）。
fn marked(
    map: &Map<String, Value>,
    mark: &Value,
    path: Option<&Vec<String>>,
) -> Result<Param, String> {
    let name = mark
        .as_str()
        .ok_or_else(|| format!("{MARK} is not a string"))?;
    if name.is_empty() || !name.bytes().all(tchar) {
        return Err(format!("{MARK} {name:?} is not a header name"));
    }
    let path = path.filter(|path| !path.is_empty()).ok_or_else(|| {
        format!("{MARK} {name:?} is not on a property reached through properties alone")
    })?;
    let primitive = map
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "string" | "integer" | "boolean"));
    if !primitive {
        return Err(format!(
            "{MARK} {name:?} is not on a string, integer or boolean"
        ));
    }
    Ok(Param {
        name: name.to_string(),
        path: path.clone(),
    })
}

/// HTTP 头名许的字（RFC 9110 的 `tchar`）。
fn tchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

/// 照 `params` 从调用参数 `arguments` 里取值，交回要带的请求头：`Mcp-Param-<名>` 和编好的值。没有值的不带。
pub(super) fn headers(params: &[Param], arguments: &Value) -> Vec<(String, String)> {
    params
        .iter()
        .filter_map(|param| {
            let value = param
                .path
                .iter()
                .try_fold(arguments, |at, key| at.get(key))?;
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Bool(yes) => yes.to_string(),
                Value::Number(number) => integer(number)?,
                _ => return None,
            };
            Some((
                format!("Mcp-Param-{}", param.name),
                super::header_value(&text),
            ))
        })
        .collect()
}

/// 整数写成十进制：模型交来 `42.0` 这样的也算整数；有小数的不算。
fn integer(number: &serde_json::Number) -> Option<String> {
    if let Some(whole) = number.as_i64() {
        return Some(whole.to_string());
    }
    if let Some(whole) = number.as_u64() {
        return Some(whole.to_string());
    }
    let float = number.as_f64()?;
    // 双精度能准确表示的整数范围（规范要的「JavaScript 安全整数」）。
    const SAFE: f64 = 9_007_199_254_740_991.0;
    (float.fract() == 0.0 && float.abs() <= SAFE).then(|| format!("{float:.0}"))
}

impl super::Remote {
    /// 调工具的请求 `message` 要带的 `Mcp-Param-<名>`：照列工具时记下的那件工具的标记取值（施工 PT-1 中）。别的请求没有。
    pub(super) fn param_headers(&self, message: &Value) -> Vec<(String, String)> {
        if message["method"] != "tools/call" {
            return Vec::new();
        }
        let name = message["params"]["name"].as_str().unwrap_or_default();
        let known = self.params.lock().unwrap_or_else(PoisonError::into_inner);
        known.get(name).map_or_else(Vec::new, |marked| {
            headers(marked, &message["params"]["arguments"])
        })
    }

    /// 列工具的一条回应 `line`：照规范查每件工具的 `x-mcp-header`，不合的剔掉、往日志写一行；合的记下镜像哪几个参数（施工 PT-1 中）。
    /// 一件都没剔的原样交回。
    pub(super) fn screen(&self, line: &str) -> String {
        let Ok(mut message) = serde_json::from_str::<Value>(line) else {
            return line.to_string();
        };
        let Some(tools) = message["result"]["tools"].as_array_mut() else {
            return line.to_string();
        };
        let mut known = self.params.lock().unwrap_or_else(PoisonError::into_inner);
        let before = tools.len();
        tools.retain(|tool| {
            let name = tool["name"].as_str().unwrap_or_default();
            match of(&tool["inputSchema"]) {
                Ok(marked) => {
                    if marked.is_empty() {
                        known.remove(name);
                    } else {
                        known.insert(name.to_string(), marked);
                    }
                    true
                }
                Err(why) => {
                    (self.log)(format!("WARN tool {name} left out: {why}"));
                    false
                }
            }
        });
        if tools.len() == before {
            return line.to_string();
        }
        message.to_string()
    }
}

#[cfg(test)]
mod tests;
