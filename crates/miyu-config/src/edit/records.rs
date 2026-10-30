//! 一组字段的列表怎么改、怎么从协议上读（施工 PC-2，设计 53 第二节第 3、5 条）。
//!
//! 改：整条一起换。用 `toml_edit` 的可改文档（[`DocumentMut`]）改这一项，别的照原样交出（它只重写动过的那几块）：
//!
//! - 原来写成表的数组 `[[<键>]]` 的：换成新的几块，第几块照原来第几块的位置和它上面的注释、空行；多出来的接在原来最后一块后面，
//!   少了的去掉。
//! - 原来写成行内数组的：原地换成行内数组，行尾注释留着。
//! - 原来没写的：新开几块 `[[<键>]]`，跟在这一项所属的那张表（`[<前缀>]`）后面。
//! - 换成空列表的：写成 `<名字> = []`，落在所属的那张表里（排在任何子表的表头前面），原来的几块去掉；表的数组写不出空的。
//! - 删掉：原来的几块、行内数组都去掉。
//!
//! 读协议上的：对象的数组，每一项照字段读，问题报到「第几项的哪个字段」。

use std::collections::BTreeMap;

use toml_edit::{ArrayOfTables, DocumentMut, InlineTable, Item as Node, Table, Value as TomlValue};

use super::from_json;
use crate::item::{Kind, RECORDS_MOST, Record};
use crate::problem::Code;
use crate::value::Value;

/// 是不是要照一组字段的列表改：要写的是一组字段的列表（不是空的），或者这一项原来写成了表的数组（换成空的、删掉都算）。
pub(super) fn applies(value: Option<&Value>, existing: Option<&Node>) -> bool {
    let records = matches!(value, Some(Value::List(values)) if values.first().is_some_and(|first| matches!(first, Value::Record(_))));
    records || matches!(existing, Some(Node::ArrayOfTables(_)))
}

/// 照 `group`、`last` 找到这一项，换成 `value`（`None` 是删掉）。读不懂、放不进去的（所属的那一格写成了值）是空的。
pub(super) fn apply(
    text: &str,
    group: &[&str],
    last: &str,
    value: Option<&Value>,
) -> Option<String> {
    let mut document: DocumentMut = text.parse().ok()?;
    let mut table = document.as_table_mut();
    for part in group {
        let next = table
            .entry(part)
            .or_insert_with(|| {
                let mut implicit = Table::new();
                implicit.set_implicit(true);
                Node::Table(implicit)
            })
            .as_table_mut()?;
        table = next;
    }
    let old = table.get(last).cloned();
    match value {
        None => {
            table.remove(last);
        }
        Some(Value::List(values)) if values.is_empty() => {
            table.remove(last);
            // 落在所属的这张表里（它原来只因为子表才有、没有表头的，放了值 `toml_edit` 就写出表头）。
            table.insert(last, Node::Value(TomlValue::Array(toml_edit::Array::new())));
        }
        Some(Value::List(values)) => match old {
            Some(Node::Value(TomlValue::Array(array))) => {
                let mut inline = toml_edit::Array::new();
                for value in values.iter() {
                    inline.push(TomlValue::InlineTable(inline_table(value)?));
                }
                inline.decor_mut().clone_from(array.decor());
                table.insert(last, Node::Value(TomlValue::Array(inline)));
            }
            old => {
                let before: Vec<Table> = match old {
                    Some(Node::ArrayOfTables(array)) => array.iter().cloned().collect(),
                    _ => Vec::new(),
                };
                let mut blocks = ArrayOfTables::new();
                for (n, value) in values.iter().enumerate() {
                    let mut block = Table::new();
                    for (name, field) in fields(value)? {
                        block.insert(name, Node::Value(toml_value(field)?));
                    }
                    if let Some(was) = before.get(n) {
                        block.decor_mut().clone_from(was.decor());
                        block.set_position(was.position());
                    } else if let Some(last_was) = before.last() {
                        block.set_position(last_was.position());
                    }
                    blocks.push(block);
                }
                table.insert(last, Node::ArrayOfTables(blocks));
            }
        },
        Some(_) => return None,
    }
    Some(document.to_string())
}

/// 一项的几个字段。
fn fields(value: &Value) -> Option<&BTreeMap<String, Value>> {
    match value {
        Value::Record(fields) => Some(fields),
        _ => None,
    }
}

/// 一项写成行内表。
fn inline_table(value: &Value) -> Option<InlineTable> {
    let mut table = InlineTable::new();
    for (name, field) in fields(value)? {
        table.insert(name, toml_value(field)?);
    }
    Some(table)
}

/// 一个值写成 TOML 的值（照 [`Value::toml`] 写好再读回来）。
fn toml_value(value: &Value) -> Option<TomlValue> {
    value.toml().parse().ok()
}

/// 协议上的一组字段的列表（对象的数组）照 `record` 读：交回读好的值，或者每一处问题（位置 `[2].key` 这样，整条的是空的；码；
/// 原文）。超过 [`RECORDS_MOST`] 项的整条报 `out_of_range`；不是数组的、某一项不是对象的报 `wrong_type`；字段照字段的类型读、查，
/// 读不成的 `wrong_type`（密钥字段只收 `{"secret": …}`、`{"env": …}`，明文也是它），少写的 `missing_field`，多写的
/// `unknown_field`。
///
/// # Errors
///
/// 每一处问题。
pub fn from_json_records(
    record: &Record,
    value: &serde_json::Value,
) -> Result<Value, Vec<(String, Code, String)>> {
    let Some(items) = value.as_array() else {
        return Err(vec![(String::new(), Code::WrongType, value.to_string())]);
    };
    if items.len() > RECORDS_MOST {
        return Err(vec![(
            String::new(),
            Code::OutOfRange,
            format!("{} items", items.len()),
        )]);
    }
    let mut problems = Vec::new();
    let mut read = Vec::new();
    for (n, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            problems.push((format!("[{n}]"), Code::WrongType, item.to_string()));
            continue;
        };
        let mut fields = BTreeMap::new();
        for (name, json) in object {
            let at = format!("[{n}].{name}");
            let Some(field) = record.fields.iter().find(|field| field.key == name) else {
                problems.push((at, Code::UnknownField, json.to_string()));
                continue;
            };
            match from_json(field.kind, json) {
                None => problems.push((at, Code::WrongType, json.to_string())),
                Some(value) => match field.kind.check(&value) {
                    Ok(()) => {
                        fields.insert(name.clone(), value);
                    }
                    Err(code) => problems.push((at, code, json.to_string())),
                },
            }
        }
        for field in record.fields {
            if field.default.is_none() && !object.contains_key(field.key) {
                problems.push((
                    format!("[{n}].{}", field.key),
                    Code::MissingField,
                    String::new(),
                ));
            }
        }
        read.push(Value::Record(fields));
    }
    match problems.is_empty() {
        true => Ok(Value::List(read.into())),
        false => Err(problems),
    }
}

/// 这一项是不是一组字段的列表：是的交回它的字段。
pub fn record_of(kind: Kind) -> Option<&'static Record> {
    match kind {
        Kind::List(Kind::Record(record)) => Some(record),
        _ => None,
    }
}
