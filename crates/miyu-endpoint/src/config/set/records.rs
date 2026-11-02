//! `config.set` 里一组字段的列表（施工 PC-2，设计 53 第二节第 5 条）：协议上的对象的数组照 JSON 读；`miyu config set` 敲的 TOML
//! 字先读成行内数组再换成 JSON（从 `set.rs` 分出来，行数）。

use miyu_config::{Kind, Value, edit};
use serde_json::{Value as Json, json};

/// 一项要改成的值照这一项的类型读出来（施工 R3S-1：扩展写的照它找有没有写成引用的）：一组字段的列表照 [`change_json`] 和
/// `config.set` 同一个读法，别的照 `edit` 的读法；读不成的是空的。
pub(in crate::config) fn wanted(
    kind: Kind,
    value: Option<&Json>,
    input: Option<&str>,
) -> Option<Value> {
    let Some(record) = edit::record_of(kind) else {
        return match (value, input) {
            (Some(json), _) => edit::from_json(kind, json),
            (None, Some(input)) => edit::input(kind, input),
            (None, None) => None,
        };
    };
    let raw = value.map_or_else(|| input.unwrap_or_default().to_string(), Json::to_string);
    let json: Json = serde_json::from_str(&raw).unwrap_or(Json::Null);
    edit::from_json_records(record, &change_json(&json, &raw)?).ok()
}

/// 一组字段的列表照协议上的 JSON 读（施工 PC-2）：`value` 给的是 JSON 原文；`input` 给的是 TOML 的字，读成行内数组再换成 JSON。
/// 读不成的是空的。
pub(super) fn change_json(json: &Json, raw: &str) -> Option<Json> {
    if !json.is_null() {
        return Some(json.clone());
    }
    let document = toml_edit::Document::parse(format!("v = {raw}")).ok()?;
    let value = document.get("v")?.as_value()?;
    Some(toml_json(value))
}

/// TOML 的值换成 JSON（`miyu config set` 敲的字）：表换对象，数组换数组。
fn toml_json(value: &toml_edit::Value) -> Json {
    match value {
        toml_edit::Value::String(text) => Json::String(text.value().clone()),
        toml_edit::Value::Integer(number) => json!(*number.value()),
        toml_edit::Value::Float(number) => json!(*number.value()),
        toml_edit::Value::Boolean(on) => Json::Bool(*on.value()),
        toml_edit::Value::Array(array) => Json::Array(array.iter().map(toml_json).collect()),
        toml_edit::Value::InlineTable(table) => Json::Object(
            table
                .iter()
                .map(|(key, value)| (key.to_string(), toml_json(value)))
                .collect(),
        ),
        toml_edit::Value::Datetime(time) => Json::String(time.value().to_string()),
    }
}
