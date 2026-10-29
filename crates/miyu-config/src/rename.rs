//! 供应商改名（施工 8-34，`docs/blueprint/models.md`「供应商改名」）：一层配置的字里，把 `[providers.<from>]` 那张表（连同
//! 子表）换成 `<to>`，再找出指着 `<from>/<模型>` 的引用、模型的列表。
//!
//! - 表的名字照 `toml_edit` 的可改文档换（[`rename_table`]）：表头、点号连着的键、行内表都认，子表跟着走；别处的字照原样
//!   交出去（`toml_edit` 读进来的格式、注释照原样留着）。换完再读一遍，读不懂的当放不进去。
//! - 引用照这一层的读法找（[`retargeted`]，同 [`crate::dangling`] 那一套）：交回要换成什么，调的一方照 `config.set` 那一套
//!   只换那一个值（[`crate::edit::apply`]）。

use toml_edit::DocumentMut;

use crate::edit::Blocked;
use crate::item::{Item, Kind, Pointed, pointed};
use crate::parse::Parsed;
use crate::value::Value;

/// 把 `text` 里 `[<group>.<from>]` 那一张表换名成 `<to>`：没有这张表的交回 `None`（字不用动）。
///
/// # Errors
///
/// 字读不懂；`<group>` 写成了别的东西；`<to>` 已经有了；换完读不懂（[`Blocked`]，带着那张表的键）。
pub fn rename_table(
    text: &str,
    group: &str,
    from: &str,
    to: &str,
) -> Result<Option<String>, Blocked> {
    let blocked = || Blocked(format!("{group}.{from}"));
    let mut document: DocumentMut = text.parse().map_err(|_| blocked())?;
    let Some(parent) = document.get_mut(group) else {
        return Ok(None);
    };
    let parent = parent.as_table_like_mut().ok_or_else(blocked)?;
    if parent.contains_key(to) {
        return Err(blocked());
    }
    let Some(table) = parent.remove(from) else {
        return Ok(None);
    };
    parent.insert(to, table);
    let renamed = document.to_string();
    match renamed.parse::<DocumentMut>() {
        Ok(_) => Ok(Some(renamed)),
        Err(_) => Err(blocked()),
    }
}

/// 一层配置 `parsed` 里，引用、模型的列表指着供应商 `from` 的：每一项交回真的键和换成 `to` 以后的整个值（列表的整个换）。
/// 只看这一层算数的项。
pub fn retargeted(items: &[Item], parsed: &Parsed, from: &str, to: &str) -> Vec<(String, Value)> {
    let moved = |text: &str| match pointed(text) {
        Some(Pointed::Provider(name)) if name == from => text
            .split_once('/')
            .map(|(_, model)| format!("{to}/{model}")),
        _ => None,
    };
    let mut found = Vec::new();
    for (key, entry) in parsed.entries.iter().filter(|(_, entry)| entry.counts) {
        let Some(item) = items.iter().find(|item| item.key == entry.item) else {
            continue;
        };
        let value = match (item.kind, &entry.value) {
            (Kind::Reference | Kind::Model | Kind::ModelOr(_), Value::Text(text)) => {
                moved(text).map(|text| Value::Text(text.into()))
            }
            (Kind::List(Kind::Reference | Kind::Model), Value::List(values)) => {
                let mut changed = false;
                let values: Vec<Value> = values
                    .iter()
                    .map(|value| match value {
                        Value::Text(text) => match moved(text) {
                            Some(text) => {
                                changed = true;
                                Value::Text(text.into())
                            }
                            None => value.clone(),
                        },
                        other => other.clone(),
                    })
                    .collect();
                changed.then_some(Value::List(values.into()))
            }
            _ => None,
        };
        if let Some(value) = value {
            found.push((key.clone(), value));
        }
    }
    found
}

#[cfg(test)]
mod tests;
