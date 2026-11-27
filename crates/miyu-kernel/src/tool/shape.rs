//! 读、写、改文件的参数的标准写法（施工 ED-3）。`edit`、`write` 为了迁就各家模型认好几种写法：顶层直接写一处（照
//! Claude Code）、`edits` 写成一个对象、`path`、`filePath`、`oldString`、`oldText`、`replaceAll` 这些别家的字段名。可照参数
//! 格式修正写成字符串的值（[`super::repair`]）、视图照参数估行数、取路径都只认标准写法：`file_path`，`edits` 是一个列表，
//! 每一项 `old_string`、`new_string`、`replace_all`。这里把读得懂的规整成标准写法。
//!
//! 只在用的时候规整：日志里的参数原文、发给模型的请求一个字节不动。值一个字节不碰，只换名字、挪位置；写成字符串的布尔
//! 规整以后到了声明过的位置，照旧交给修正还原。读不懂的（同一格写了两个名字、不是一个对象）原样交回，照原文报错。
//! 只给文件这几件用：`grep`、`show` 的 `path` 是它们自己的标准名。

use std::borrow::Cow;

use serde_json::{Map, Value};

/// 路径：标准名和别名。
const PATH: (&str, &[&str]) = ("file_path", &["path", "filePath"]);

/// 要改的一处的几格：标准名和别名。
const CHANGE: [(&str, &[&str]); 3] = [
    ("old_string", &["oldString", "oldText"]),
    ("new_string", &["newString", "newText"]),
    ("replace_all", &["replaceAll"]),
];

/// 参数原文规整成标准写法；本来就是标准写法的、读不懂的照原文。
#[must_use]
pub fn standard(args: &str) -> Cow<'_, str> {
    let Ok(mut value) = serde_json::from_str::<Value>(args) else {
        return Cow::Borrowed(args);
    };
    if standard_value(&mut value) {
        Cow::Owned(value.to_string())
    } else {
        Cow::Borrowed(args)
    }
}

/// 同 [`standard`]，一份解开的参数，就地规整。改过的交回真；读不懂的一格不动、交回假。
pub fn standard_value(args: &mut Value) -> bool {
    let Value::Object(object) = args else {
        return false;
    };
    if !understood(object) {
        return false;
    }
    let mut changed = rename(object, PATH);
    match object.get_mut("edits") {
        Some(Value::Object(change)) => {
            rename_change(change);
            let change = Value::Object(std::mem::take(change));
            object.insert("edits".to_string(), Value::Array(vec![change]));
            changed = true;
        }
        Some(Value::Array(changes)) => {
            for change in changes.iter_mut().filter_map(Value::as_object_mut) {
                changed |= rename_change(change);
            }
        }
        Some(Value::Null) | None => return lift(object) | changed,
        // 写成字符串、修正还原不了的：不是这里读得懂的，照原样交给工具报错。
        Some(_) => return changed,
    }
    // 有 `edits` 时顶层的 `replace_all` 本来就不算数：去掉，写成字符串的不再报错。
    for name in [CHANGE[2].0].iter().chain(CHANGE[2].1) {
        changed |= object.remove(*name).is_some();
    }
    changed
}

/// 读得懂：路径、要改的每一处，同一格都只写了一个名字。
fn understood(object: &Map<String, Value>) -> bool {
    let once = |object: &Map<String, Value>, (name, aliases): (&str, &[&str])| {
        std::iter::once(&name)
            .chain(aliases)
            .filter(|name| object.contains_key(**name))
            .count()
            <= 1
    };
    let change = |object: &Map<String, Value>| CHANGE.iter().all(|field| once(object, *field));
    once(object, PATH)
        && match object.get("edits") {
            Some(Value::Object(one)) => change(one),
            Some(Value::Array(many)) => many.iter().filter_map(Value::as_object).all(change),
            _ => change(object),
        }
}

/// 别名换成标准名，值原样。换过的交回真。
fn rename(object: &mut Map<String, Value>, (name, aliases): (&str, &[&str])) -> bool {
    for alias in aliases {
        if let Some(value) = object.remove(*alias) {
            object.insert(name.to_string(), value);
            return true;
        }
    }
    false
}

/// 要改的一处的几格换成标准名。
fn rename_change(change: &mut Map<String, Value>) -> bool {
    CHANGE
        .iter()
        .fold(false, |changed, field| rename(change, *field) | changed)
}

/// 顶层直接写一处的：`old_string`、`new_string` 都在的，连着 `replace_all` 收进 `edits` 的一项。缺一样的只换名字，照原样
/// 交给工具（说没给要改的）。
fn lift(object: &mut Map<String, Value>) -> bool {
    let mut changed = rename_change(object);
    if object.contains_key(CHANGE[0].0) && object.contains_key(CHANGE[1].0) {
        let mut change = Map::new();
        for (name, _) in CHANGE {
            if let Some(value) = object.remove(name) {
                change.insert(name.to_string(), value);
            }
        }
        object.insert(
            "edits".to_string(),
            Value::Array(vec![Value::Object(change)]),
        );
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests;
