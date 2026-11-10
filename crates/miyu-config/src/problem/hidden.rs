//! 密钥项写错时不抄原文（施工 SEC-1，`docs/blueprint/config.md`「报错」`got` 那一行）：类型是密钥、密钥的列表、带密钥字段的
//! 列表的一格，问题里不记收到的原文，只记它是什么样的（[`Shape`]：几个字符的字、几项的列表、一张表），说成话时照连接的语言
//! 说。没有类型的一格（不认识的键、列表里不认识的字段）同样不抄：拼错了键的密钥项也不漏。别的项照旧照抄。
//!
//! 为什么在记问题时就丢掉：原文先截到 80 个字符才进 `got`，到说话时再藏，长的数不对、截出来的前半段已经在问题里了；问题也会
//! 被克隆、比较、交给好几处说，原文不进来，哪一处都漏不出去。

use serde_json::Value as Json;
use toml_edit::{Document, Value as TomlValue};

use crate::item::{Item, Kind};
use crate::words::{Missing, Words, sentence};

/// 收到的是什么样的：只说种类和多长，不说内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// 一串字，几个字符（引号、转义去掉以后数）。
    Text(usize),
    /// 一个列表，几项。
    List(usize),
    /// 一张表（对象）。
    Table,
    /// 别的值（数、开关、日期），原文几个字符。
    Value(usize),
}

impl Shape {
    /// 原文 `raw` 是什么样的：先照 JSON 读（`config.set` 的 `value`），再照 TOML 的一个值读（配置文件、带引号的人敲的），都读
    /// 不成的当一串字（人敲的不带引号的）。
    pub fn of(raw: &str) -> Shape {
        if let Ok(json) = serde_json::from_str::<Json>(raw) {
            return match json {
                Json::String(text) => Shape::Text(text.chars().count()),
                Json::Array(values) => Shape::List(values.len()),
                Json::Object(_) => Shape::Table,
                _ => Shape::Value(raw.chars().count()),
            };
        }
        // 一行里只能有这一个值：`"a"\nb = 1` 这样多出来的不算。
        let document = Document::parse(format!("v = {raw}"))
            .ok()
            .filter(|document| document.as_table().len() == 1);
        match document
            .as_ref()
            .and_then(|document| document.get("v")?.as_value())
        {
            Some(TomlValue::String(text)) => Shape::Text(text.value().chars().count()),
            Some(TomlValue::Array(values)) => Shape::List(values.len()),
            Some(TomlValue::InlineTable(_)) => Shape::Table,
            Some(_) => Shape::Value(raw.chars().count()),
            None => Shape::Text(raw.chars().count()),
        }
    }

    /// 说成给人看的话（`config/got/…`）。
    ///
    /// # Errors
    ///
    /// 要用的字缺了。
    pub fn said(self, words: &dyn Words) -> Result<String, Missing> {
        match self {
            Shape::Text(count) => counted(words, "config/got/text", count),
            Shape::List(count) => counted(words, "config/got/list", count),
            Shape::Table => sentence(words, "config/got/table", &[]),
            Shape::Value(count) => counted(words, "config/got/value", count),
        }
    }
}

/// 这种类型里有没有密钥：本身是密钥、元素是密钥、字段里有密钥的。
pub fn holds_secret(kind: Kind) -> bool {
    match kind {
        Kind::Secret => true,
        Kind::List(inner) => holds_secret(*inner),
        Kind::Record(record) => record.fields.iter().any(|field| holds_secret(field.kind)),
        _ => false,
    }
}

/// 问题的键 `key` 说的那一格是不是密钥（照清单 `items`）：`<键>[第几项].<字段>` 看那个字段，`<键>[第几项]`、`<键>` 看整项。
/// 清单里没有的不是。
pub fn secret_at(items: &[Item], key: &str) -> bool {
    let kind = match key.split_once('[') {
        Some((_, rest)) if rest.contains("].") => {
            crate::key::described_by(items, key).map(|field| field.kind)
        }
        Some((list, _)) => crate::key::item_of(items, list).map(|item| item.kind),
        None => crate::key::item_of(items, key).map(|item| item.kind),
    };
    kind.is_some_and(holds_secret)
}

/// 带数的一句。
fn counted(words: &dyn Words, said: &str, count: usize) -> Result<String, Missing> {
    sentence(words, said, &[("count", &count.to_string())])
}

#[cfg(test)]
mod tests;
