//! 照真的模型那样回整理记忆的三种请求（施工 LW-14，排函数的剧本抽记忆那一步用）：抽取、写关键词、合并。只有给了
//! [`super::Fake::organize`] 的才这样回；别的几场不开，回的仍然是那一段占地方的字（核心读不出结果，记一行失败）。
//!
//! 照出厂指令里的一句认是哪一种（`resources/software/memory/` 下的 `extract/instruction.txt`、`tag/instruction.txt`、
//! `merge/instruction.txt`），回的 JSON 照指令里写的形状：抽取的照请求里第一轮的编号记一条，写关键词的给请求里列的每一条
//! 写一行，合并的不改不作废、写一段摘要。指令改了这几句，这里跟着改；认不出时假模型回占地方的字，剧本等不到记下的
//! 结果就报错（`order/script.rs`）。

use serde_json::{Value, json};

/// 抽取请求认的那一句。
pub const EXTRACT: &str = "Find what is worth remembering in the conversation below";

/// 写关键词请求认的那一句。
pub const TAG: &str = "write one line of at most 15 comma-separated search keywords";

/// 合并请求认的那一句。
pub const MERGE: &str = "Tidy them up.";

/// 抽取记下的那一条的字。
const REMEMBERED: &str = "The owner measures how much memory Miyu uses.";

/// 照请求体 `body`（openai-chat 的 JSON）回整理记忆的回答正文；不是这三种的、读不出来的是 `None`。
pub fn answer(body: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let mut text = String::new();
    strings(&value["messages"], &mut text);
    if text.contains(EXTRACT) {
        let turn = first_turn(&text)?;
        let memory = json!({"class": "user", "text": REMEMBERED, "turn": turn});
        Some(json!({"memories": [memory]}).to_string())
    } else if text.contains(TAG) {
        let keywords: Vec<Value> = ids(&text)
            .into_iter()
            .map(|id| json!({"id": id, "keywords": "memory, measuring, Miyu"}))
            .collect();
        Some(json!({"keywords": keywords}).to_string())
    } else if text.contains(MERGE) {
        let summary = "I know my owner measures how much memory Miyu uses.";
        Some(json!({"revised": [], "retired": [], "summary": summary}).to_string())
    } else {
        None
    }
}

/// `value` 里所有的字照先后接起来（消息的正文可能是一段字，也可能是几块）。
fn strings(value: &Value, out: &mut String) {
    match value {
        Value::String(text) => {
            out.push_str(text);
            out.push('\n');
        }
        Value::Array(items) => items.iter().for_each(|item| strings(item, out)),
        Value::Object(fields) => fields.values().for_each(|field| strings(field, out)),
        _ => {}
    }
}

/// 抽取请求里第一轮的编号：`<turn number="42" …>` 里的 42（`extract/turn-open.txt`）。
fn first_turn(text: &str) -> Option<u64> {
    let rest = &text[text.find("<turn number=\"")? + "<turn number=\"".len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// 写关键词请求里列的几条的编号：一行一条，开头是 `m<数>`（`memory_search/memory.txt`）。
fn ids(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|word| {
            word.strip_prefix('m').is_some_and(|digits| {
                !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
            })
        })
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests;
