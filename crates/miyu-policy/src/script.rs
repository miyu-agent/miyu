//! 脚本的开头注释（施工 X-12 上，`docs/blueprint/skills.md`「脚本」第 1、2 条）：照旧版的写法读，旧版的脚本照搬就能用。
//!
//! 文件开头连成一段的注释行（`#` 或 `//` 开头，第一行的 `#!` 也算在里面）是开头，第一行不是注释的地方结束。认这几个键（冒号全角、
//! 半角都行）：
//!
//! - `Description`：给模型看的一句，必须有；
//! - `Display name`、`显示名称`：给人看的名字（英文、中文），`描述`：给人看的一句说明（中文）；
//! - `Timeout`：秒，正整数，没写的 [`TIMEOUT`]；
//! - `Parameters`：参数的 JSON Schema，接在下面的几行注释里，拼成一份 JSON 为止；顶层得是 `"type": "object"`。没写的当
//!   没有参数；
//! - `Need load`：`yes` 和没写的要先加载，`no` 是全量（施工 L-3，`25-工具加载.md` W9，2026-10-20 项目主人改：脚本默认先加载，
//!   原来没写的全量）；大小写不论，`true`、`false` 也认。个人设置的 `loading.tools.<工具名>` 盖过它。
//!
//! 别的键（`Id`、`Group`、`Trust`、`Permission`、编码声明……）照旧版留着的，认、不用。工具名照文件名（[`tool_name`]）。纯逻辑：
//! 进来的是文件名和文件里的字，出去的是读好的样子，或者哪里不对。

use std::collections::BTreeMap;
use std::fmt;

use serde_json::Value;

/// 没写 `Timeout` 的等多少秒（同旧版）。
pub const TIMEOUT: u64 = 60;

/// 工具名最多几个字（工具目录的上限）。
const NAME_CHARS: usize = 64;

/// 脚本算的那个功能的编号的开头（施工 X-12 中）：`script-<工具名>`，同技能的 `skill-<名字>`。
const FEATURE_PREFIX: &str = "script-";

/// 工具名是 `name` 的脚本算的功能的编号（施工 X-12 中）：预设照它开关。
pub fn feature_id(name: &str) -> String {
    format!("{FEATURE_PREFIX}{name}")
}

/// 没写参数的：一个参数都没有。
const NO_PARAMETERS: &str = r#"{"type":"object","properties":{}}"#;

/// 读好的开头。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptHead {
    /// 工具名：照文件名（[`tool_name`]）。
    pub name: String,
    /// 给模型看的一句（`Description`）。
    pub description: String,
    /// 参数的 JSON Schema，原样的字（没写的是一个参数都没有的那一份）。
    pub parameters: String,
    /// 等多少秒。
    pub timeout: u64,
    /// 给人看的名字，照语言：`en` 是 `Display name`，`zh` 是 `显示名称`。
    pub display_name: BTreeMap<String, String>,
    /// 给人看的一句说明，照语言：`zh` 是 `描述`。
    pub summary: BTreeMap<String, String>,
    /// 要先加载（`Need load`，施工 L-1）。
    pub need_load: bool,
}

/// 哪里不对。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptProblem {
    /// 文件名换成工具名以后是空的。
    NoName,
    /// 没写 `Description`，或者是空的。
    NoDescription,
    /// `Timeout` 不是正整数：写的原样。
    BadTimeout(String),
    /// `Parameters` 拼不成 JSON：解析器的原话。
    BadParameters(String),
    /// `Parameters` 顶层不是 `"type": "object"`。
    NotAnObject,
    /// `Need load` 不是 `yes`、`no`（施工 L-1）：写的原样。
    BadNeedLoad(String),
}

impl fmt::Display for ScriptProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptProblem::NoName => f.write_str("the file name gives no tool name"),
            ScriptProblem::NoDescription => f.write_str("Description is missing or empty"),
            ScriptProblem::BadTimeout(text) => {
                write!(f, "Timeout {text:?} is not a positive number of seconds")
            }
            ScriptProblem::BadParameters(why) => write!(f, "Parameters is not valid JSON: {why}"),
            ScriptProblem::NotAnObject => f.write_str("Parameters must be a JSON object schema"),
            ScriptProblem::BadNeedLoad(text) => write!(f, "Need load {text:?} is not yes or no"),
        }
    }
}

/// 读文件名是 `file_name` 的脚本的字 `text`。
///
/// # Errors
///
/// 见 [`ScriptProblem`]。
pub fn read(file_name: &str, text: &str) -> Result<ScriptHead, ScriptProblem> {
    let name = tool_name(file_name).ok_or(ScriptProblem::NoName)?;
    let lines = comments(text);
    let mut head = ScriptHead {
        name,
        description: String::new(),
        parameters: NO_PARAMETERS.to_string(),
        timeout: TIMEOUT,
        display_name: BTreeMap::new(),
        summary: BTreeMap::new(),
        need_load: true,
    };
    let mut rest = lines.iter();
    while let Some(line) = rest.next() {
        let Some((key, value)) = split(line) else {
            continue;
        };
        match key {
            "Description" => head.description = value.to_string(),
            "Display name" => put(&mut head.display_name, "en", value),
            "显示名称" => put(&mut head.display_name, "zh", value),
            "描述" => put(&mut head.summary, "zh", value),
            "Timeout" => {
                head.timeout = value
                    .parse::<u64>()
                    .ok()
                    .filter(|seconds| *seconds > 0)
                    .ok_or_else(|| ScriptProblem::BadTimeout(value.to_string()))?;
            }
            "Parameters" => head.parameters = parameters(value, &mut rest)?,
            "Need load" => head.need_load = yes_or_no(value)?,
            _ => {}
        }
    }
    if head.description.is_empty() {
        return Err(ScriptProblem::NoDescription);
    }
    Ok(head)
}

/// `Need load` 的值：`yes`、`true` 是要，`no`、`false` 是不要，大小写不论（施工 L-1）。
fn yes_or_no(value: &str) -> Result<bool, ScriptProblem> {
    match value.to_ascii_lowercase().as_str() {
        "yes" | "true" => Ok(true),
        "no" | "false" => Ok(false),
        _ => Err(ScriptProblem::BadNeedLoad(value.to_string())),
    }
}

/// 文件名换成工具名：去掉最后一个扩展名（`.py`、`.sh` 这种；没有的照原样），不合 `[A-Za-z0-9_-]` 的字换成 `_`，最多
/// 64 个字（工具目录的上限）。点开头的、换完是空的没有。
pub fn tool_name(file_name: &str) -> Option<String> {
    if file_name.starts_with('.') {
        return None;
    }
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let name: String = stem
        .chars()
        .take(NAME_CHARS)
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => c,
            _ => '_',
        })
        .collect();
    (!name.is_empty()).then_some(name)
}

/// 开头那一段注释，去掉注释符和它后面的一个空格、行尾的 `\r`；文件开头的 BOM 不算。第一行的 `#!` 也是 `#` 开头，没有键，
/// 不用另外跳过。
fn comments(text: &str) -> Vec<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    text.lines()
        .map(|line| line.trim_end_matches('\r'))
        .map_while(|line| {
            line.strip_prefix("//")
                .or_else(|| line.strip_prefix('#'))
                .map(|rest| rest.strip_prefix(' ').unwrap_or(rest))
        })
        .collect()
}

/// 一行注释拆成键和值：键在第一个冒号（全角、半角）前面，两头去掉空白；不是这样的一行没有。
fn split(line: &str) -> Option<(&str, &str)> {
    let at = line.find([':', '：'])?;
    let colon = line[at..].chars().next()?.len_utf8();
    let key = line[..at].trim();
    (!key.is_empty() && !key.contains(['{', '"'])).then(|| (key, line[at + colon..].trim()))
}

/// 一格给人看的字：空的不记。
fn put(map: &mut BTreeMap<String, String>, language: &str, value: &str) {
    if !value.is_empty() {
        map.insert(language.to_string(), value.to_string());
    }
}

/// `Parameters` 那一格：同一行写了的照它，没写的接着往下一行行拼，拼成一份 JSON 为止；顶层得是对象、`type` 是 `object`。
/// 交回整理成一行的写法。
fn parameters(first: &str, rest: &mut std::slice::Iter<'_, &str>) -> Result<String, ScriptProblem> {
    let mut text = first.to_string();
    let parsed = loop {
        if !text.trim().is_empty() {
            match serde_json::from_str::<Value>(&text) {
                Ok(value) => break value,
                Err(error) if !error.is_eof() => {
                    return Err(ScriptProblem::BadParameters(error.to_string()));
                }
                Err(_) => {}
            }
        }
        let Some(line) = rest.next() else {
            return Err(ScriptProblem::BadParameters(
                "the JSON is not finished".to_string(),
            ));
        };
        text.push('\n');
        text.push_str(line);
    };
    if parsed.get("type").and_then(Value::as_str) != Some("object") {
        return Err(ScriptProblem::NotAnObject);
    }
    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests;
