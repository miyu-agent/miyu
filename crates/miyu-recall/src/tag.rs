//! 写关键词交回来的怎么读（施工 R-15 下，`docs/blueprint/memory.md` 第二条）：模型交回一个 JSON 对象
//! `{"keywords": [{"id", "keywords"}]}`。纯逻辑：调的一方把这一次交进去的编号交进来。
//!
//! - 照规矩取：从第一个 `{` 到最后一个 `}`，前后多说的话不管；读不成 JSON 对象的、没有 `keywords` 列表的，整次算失败。
//! - 一条不合的丢掉、别的照收：编号不是这一次交进去的、关键词不是字或整理以后是空的；同一条写了两次的照头一次。关键词照
//!   [`keywords_of`] 整理（去掉前后空白，太长的在逗号处截断）。

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use serde_json::Value;

use crate::memory::{MemoryId, keywords_of};

/// 读交回来的字：这一次交进去的是 `given` 这几条。交回（编号、关键词），照交回来的先后。
///
/// # Errors
///
/// 找不到 `{…}`、读不成 JSON 对象、没有 `keywords` 列表：英文的一句为什么。
pub fn keywords(
    answer: &str,
    given: &BTreeSet<MemoryId>,
) -> Result<Vec<(MemoryId, String)>, String> {
    let (Some(start), Some(end)) = (answer.find('{'), answer.rfind('}')) else {
        return Err("no JSON object in the answer".to_string());
    };
    if end < start {
        return Err("no JSON object in the answer".to_string());
    }
    let value: Value = serde_json::from_str(&answer[start..=end])
        .map_err(|error| format!("answer not readable: {error}"))?;
    let listed = value["keywords"]
        .as_array()
        .ok_or_else(|| "no keywords list in the answer".to_string())?;
    let mut found: Vec<(MemoryId, String)> = Vec::new();
    for item in listed {
        let Some(id) = item["id"].as_str().and_then(MemoryId::parse) else {
            continue;
        };
        if !given.contains(&id) || found.iter().any(|(seen, _)| *seen == id) {
            continue;
        }
        if let Some(keywords) = item["keywords"].as_str().and_then(keywords_of) {
            found.push((id, keywords));
        }
    }
    Ok(found)
}
