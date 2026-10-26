//! 出厂的组表 `resources/packages/groups.toml`（施工 BS-1 下，设计 `44-基础系统拆包.md` 第三节第 2 条）：一个组一段，段名是
//! 编号（写法同包的编号），`name` 必写、`summary` 可写，都是语言表。组里有哪些包由包的清单写（`[package] groups`）。

use toml_edit::DocumentMut;

use crate::phrases::{self, Phrases};

/// 一个组。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// 编号。
    pub id: String,
    /// 名字。
    pub name: Phrases,
    /// 说明；没写的是空的。
    pub summary: Phrases,
}

/// 读组表的原文 `text`，照写的先后。
///
/// # Errors
///
/// 写错的：说哪一段、哪一格不对（英文一句，给写资源的人看）。
pub fn read(text: &str) -> Result<Vec<Group>, String> {
    let document: DocumentMut = text
        .parse()
        .map_err(|error: toml_edit::TomlError| error.to_string())?;
    let mut groups = Vec::new();
    for (id, item) in document.iter() {
        let Some(table) = item.as_table_like() else {
            return Err(format!("{id}: not a table"));
        };
        if !crate::key::valid(crate::key::ID, id) {
            return Err(format!("{id}: not a valid id"));
        }
        if let Some((key, _)) = table
            .iter()
            .find(|(key, _)| !matches!(*key, "name" | "summary"))
        {
            return Err(format!("{id}.{key}: unknown key"));
        }
        let phrases = |key: &str| match table.get(key) {
            Some(item) => phrases::read(item).map_err(|error| format!("{id}.{key}: {error:?}")),
            None => Ok(Phrases::new()),
        };
        let name = phrases("name")?;
        if name.is_empty() {
            return Err(format!("{id}.name: missing"));
        }
        groups.push(Group {
            id: id.to_string(),
            name,
            summary: phrases("summary")?,
        });
    }
    Ok(groups)
}

#[cfg(test)]
mod tests;
