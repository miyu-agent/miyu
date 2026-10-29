//! 工具说明的同名覆盖（施工 OV-1 上，设计 00 第五节「可自定义」、`25-工具加载.md` W7）：`tools/<工具>.toml` 只写改了的项，没写的
//! 照出厂。纯逻辑：读成一份、换进工具面上的那一件；文件在哪由存储那一层找（只有数据根一处，不叠）。
//!
//! ```toml
//! description = "Read a file. ..."
//! [parameters.path]
//! description = "Absolute path."
//! ```
//!
//! 给模型看的字照规矩查（英文、长度）；参数名必须是这件工具参数格式里有的。写坏的整份不认，照下一层。

use std::collections::BTreeMap;

use toml_edit::{DocumentMut, Item};

use crate::tools::ToolEntry;

/// 工具说明最多几个字。
const DESCRIPTION_MOST: usize = 2000;

/// 参数说明最多几个字。
const PARAMETER_MOST: usize = 500;

/// 一件工具的覆盖：没写的格是 `None`、空的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolOverride {
    /// 工具说明。
    pub description: Option<String>,
    /// 参数名到它的说明。
    pub parameters: BTreeMap<String, String>,
}

impl ToolOverride {
    /// 读一份：不是这个样子、写了不认识的格、给模型看的字不合规矩的交回为什么。
    ///
    /// # Errors
    ///
    /// 见上。
    pub fn parse(text: &str) -> Result<ToolOverride, String> {
        let document: DocumentMut = text.parse().map_err(|error| format!("{error}"))?;
        let mut read = ToolOverride::default();
        for (key, item) in document.iter() {
            match key {
                "description" => {
                    let text = item.as_str().ok_or("description must be a string")?;
                    model_text(text, DESCRIPTION_MOST)
                        .map_err(|why| format!("description {why}"))?;
                    read.description = Some(text.to_string());
                }
                "parameters" => {
                    let table = item.as_table_like().ok_or("parameters must be a table")?;
                    for (name, one) in table.iter() {
                        read.parameters
                            .insert(name.to_string(), parameter(name, one)?);
                    }
                }
                other => return Err(format!("unknown key {other}")),
            }
        }
        Ok(read)
    }

    /// 换进工具面上的 `entry`：参数说明改进参数格式里那个参数的 `description`。参数格式里没有这个参数的交回为什么，`entry`
    /// 不动。没写参数说明的不重写参数格式，字节照旧。
    ///
    /// # Errors
    ///
    /// 见上。
    pub fn apply(&self, entry: &mut ToolEntry) -> Result<(), String> {
        let parameters = match self.parameters.is_empty() {
            true => None,
            false => Some(described(entry.parameters.get(), &self.parameters)?),
        };
        if let Some(description) = &self.description {
            entry.description = description.clone();
        }
        if let Some(parameters) = parameters {
            entry.parameters = parameters;
        }
        Ok(())
    }
}

/// `[parameters.<name>]` 那一格：只认 `description`。
fn parameter(name: &str, item: &Item) -> Result<String, String> {
    let table = item
        .as_table_like()
        .ok_or_else(|| format!("parameters.{name} must be a table"))?;
    let mut description = None;
    for (key, value) in table.iter() {
        match key {
            "description" => description = value.as_str().map(str::to_string),
            other => return Err(format!("unknown key parameters.{name}.{other}")),
        }
    }
    let description =
        description.ok_or_else(|| format!("parameters.{name}.description must be a string"))?;
    model_text(&description, PARAMETER_MOST)
        .map_err(|why| format!("parameters.{name}.description {why}"))?;
    Ok(description)
}

/// 给模型看的字的规矩：不是空的，最多 `most` 个字，除了换行没有控制字符，CJK 的字不到一半。包的会话状态的 `note` 也照它查
/// （施工 TW-1）。
///
/// # Errors
///
/// 不合的交回哪一条。
pub fn model_text(text: &str, most: usize) -> Result<(), &'static str> {
    let count = text.chars().count();
    if text.trim().is_empty() {
        return Err("is empty");
    }
    if count > most {
        return Err("is too long");
    }
    if text.chars().any(|c| c.is_control() && c != '\n') {
        return Err("has control characters");
    }
    let cjk = text.chars().filter(|c| is_cjk(*c)).count();
    if cjk * 2 >= count {
        return Err("is not English");
    }
    Ok(())
}

/// 中日韩的字（统一表意文字、假名、谚文）。
fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}' | '\u{f900}'..='\u{faff}')
}

/// 参数格式 `schema` 里，`descriptions` 列的参数换上说明；没有的参数交回为什么。
fn described(
    schema: &str,
    descriptions: &BTreeMap<String, String>,
) -> Result<miyu_kernel::raw::RawJson, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(schema).map_err(|error| error.to_string())?;
    let properties = value
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("the tool has no parameters")?;
    for (name, description) in descriptions {
        let property = properties
            .get_mut(name)
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| format!("the tool has no parameter {name}"))?;
        property.insert(
            "description".to_string(),
            serde_json::Value::String(description.clone()),
        );
    }
    miyu_kernel::raw::RawJson::of(&value).ok_or_else(|| "parameters not written".to_string())
}

#[cfg(test)]
mod tests;
