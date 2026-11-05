//! 扩展的工具在 `provide` 规格里写的两样声明（施工 XG-1 上，设计 `63-扩展工具的效果和声明.md` 第二节）：
//!
//! - `paths`：哪几个参数是路径、是读是写。核心照它报 [`miyu_tool::Target`]，权限链和内置 `edit` 走同一张判法表（第三节第 1 条）；
//! - `background_parameters`：只在会话有后台时才给的几个参数，后台运行关着的会话工具面上去掉（第六节第 1 条）。
//!
//! 两格都照 `input_schema` 顶层的 `properties` 查：参数要在那里。写错的回 `bad_tool`，`problem` 是格名。不写的照没有，规格、
//! 工具面、登记缓存和以前逐字节一样。

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;

use miyu_kernel::tool::Access;
use miyu_tool::Target;

use super::bad_tool;
use crate::refusal::Refusal;

/// 一个是路径的参数（`paths` 的一项）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PathParameter {
    /// 参数名：`input_schema.properties` 顶层的一个，类型是字或字的数组。
    pub(super) parameter: String,
    /// 要写（新建、改、删）；不写是读。写了的，工具的访问类别要是 `write`。
    #[serde(default)]
    pub(super) write: bool,
}

/// 查 `paths`：一个数组，每项的参数在 `schema` 顶层、类型是字或字的数组、不重复；要写的要 `access` 是写。不写的是空的。
pub(super) fn paths(
    tool: &str,
    written: Option<&Value>,
    schema: &Value,
    access: &Access,
) -> Result<Vec<PathParameter>, Refusal> {
    let Some(written) = written else {
        return Ok(Vec::new());
    };
    let refused = || bad_tool(tool, "paths");
    let declared =
        serde_json::from_value::<Vec<PathParameter>>(written.clone()).map_err(|_| refused())?;
    let mut names = BTreeSet::new();
    for one in &declared {
        let pathlike = property(schema, &one.parameter).is_some_and(textual);
        if !pathlike || !names.insert(one.parameter.as_str()) {
            return Err(refused());
        }
        if one.write && *access != Access::Write {
            return Err(refused());
        }
    }
    Ok(declared)
}

/// 查 `background_parameters`：字的数组，每个是 `schema` 顶层的参数、不在 `required` 里（去掉了请求就对不上格式）、不重复。
/// 不写的是空的。
pub(super) fn background(
    tool: &str,
    written: Option<&Value>,
    schema: &Value,
) -> Result<Vec<String>, Refusal> {
    let Some(written) = written else {
        return Ok(Vec::new());
    };
    let refused = || bad_tool(tool, "background_parameters");
    let names = serde_json::from_value::<Vec<String>>(written.clone()).map_err(|_| refused())?;
    let required: Vec<&str> = schema["required"]
        .as_array()
        .map(|required| required.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut distinct = BTreeSet::new();
    for name in &names {
        if property(schema, name).is_none()
            || required.contains(&name.as_str())
            || !distinct.insert(name.as_str())
        {
            return Err(refused());
        }
    }
    Ok(names)
}

/// 照声明从参数 `args`（一个 JSON 对象的原文）里取要碰的路径：字的一条，字的数组每项一条，照声明的先后。参数缺的、不是字的
/// 不出（同内置工具「参数不对交回空的」，执行时由扩展报错）。
pub(super) fn targets(declared: &[PathParameter], args: &str) -> Vec<Target> {
    if declared.is_empty() {
        return Vec::new();
    }
    let Ok(args) = serde_json::from_str::<Value>(args) else {
        return Vec::new();
    };
    let target = |path: &str, write: bool| Target {
        path: path.to_string(),
        write,
        itself: false,
    };
    let mut targets = Vec::new();
    for one in declared {
        match &args[one.parameter.as_str()] {
            Value::String(path) => targets.push(target(path, one.write)),
            Value::Array(paths) => targets.extend(
                paths
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|path| target(path, one.write)),
            ),
            _ => {}
        }
    }
    targets
}

/// `schema` 顶层 `properties` 里叫 `name` 的那一格；没有的是没有。
fn property<'a>(schema: &'a Value, name: &str) -> Option<&'a Value> {
    schema["properties"].as_object()?.get(name)
}

/// 类型是字（`"type": "string"`），或者每项是字的数组。
fn textual(property: &Value) -> bool {
    match property["type"].as_str() {
        Some("string") => true,
        Some("array") => property["items"]["type"].as_str() == Some("string"),
        _ => false,
    }
}
