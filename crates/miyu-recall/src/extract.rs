//! 抽取交回来的怎么读（施工 R-6 上，`docs/blueprint/memory.md` 第六条第 4 款）：模型交回一个 JSON 对象
//! `{"memories": [{"class", "text", "turn", "about", "who", "people"}]}`，零条是常态（`who` 是认识的人那一类说的是谁，施工 E-1 上；
//! `people` 是发生过的事那一类涉及谁，施工 E-1 修四）。纯逻辑：调的一方把这一段有哪几轮、一条最多几个字交进来。
//!
//! - 照规矩取：从第一个 `{` 到最后一个 `}`，前后多说的话不管（模型爱包一层说明、代码块）；这一段读不成 JSON 对象、没有
//!   `memories` 列表的，整次算失败。
//! - 一条不合的丢掉、别的照收：类不在出厂四类里、正文空的或超了字数、`turn` 不是这一段里的一轮。`about` 不是
//!   `YYYY-MM-DD` 的当没写，这一条照收。

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use serde_json::Value;

use crate::memory::CLASSES;

/// 一条候选。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// 类：出厂四类之一。
    pub class: String,
    /// 正文，去掉了前后空白。
    pub text: String,
    /// 出处那一轮的编号（回合编号的数）：一定是交进来的那几轮之一。
    pub turn: u64,
    /// 说的是哪天的事（`YYYY-MM-DD`）；没写的、写法不对的没有。
    pub about: Option<String>,
    /// 说的是谁（施工 E-1 上）：认识的人那一类（`person`）交回的名字，去掉了前后空白；调的一方照这一段的说话人换成平台编号。
    /// 别的类、没写的没有。
    pub who: Option<String>,
    /// 涉及谁（施工 E-1 修四）：发生过的事那一类（`episode`）交回的几个名字，去掉了前后空白、去了重、最多 [`PEOPLE`] 个；调的
    /// 一方照这一段的说话人换成平台编号。别的类、没写的是空的。
    pub people: Vec<String>,
}

/// 一条发生过的事最多记几个涉及的人。
pub const PEOPLE: usize = 8;

/// 读交回来的字：这一段有 `turns` 这几轮，一条最多 `max_chars` 个字。
///
/// # Errors
///
/// 找不到 `{…}`、读不成 JSON 对象、没有 `memories` 列表：英文的一句为什么。
pub fn candidates(
    answer: &str,
    turns: &BTreeSet<u64>,
    max_chars: usize,
) -> Result<Vec<Candidate>, String> {
    let (Some(start), Some(end)) = (answer.find('{'), answer.rfind('}')) else {
        return Err("no JSON object in the answer".to_string());
    };
    if end < start {
        return Err("no JSON object in the answer".to_string());
    }
    let value: Value = serde_json::from_str(&answer[start..=end])
        .map_err(|error| format!("answer not readable: {error}"))?;
    let listed = value["memories"]
        .as_array()
        .ok_or_else(|| "no memories list in the answer".to_string())?;
    Ok(listed
        .iter()
        .filter_map(|item| one(item, turns, max_chars))
        .collect())
}

/// 一条：不合的是 `None`。
fn one(item: &Value, turns: &BTreeSet<u64>, max_chars: usize) -> Option<Candidate> {
    let class = item["class"].as_str()?;
    if !CLASSES.contains(&class) {
        return None;
    }
    let text = item["text"].as_str()?.trim();
    if text.is_empty() || text.chars().count() > max_chars {
        return None;
    }
    let turn = item["turn"].as_u64().filter(|turn| turns.contains(turn))?;
    let about = item["about"].as_str().filter(|about| is_date(about));
    let who = item["who"]
        .as_str()
        .map(str::trim)
        .filter(|who| class == crate::memory::PERSON && !who.is_empty());
    let people = match class == crate::memory::EPISODE {
        true => people(&item["people"]),
        false => Vec::new(),
    };
    Some(Candidate {
        class: class.to_string(),
        text: text.to_string(),
        turn,
        about: about.map(str::to_string),
        who: who.map(str::to_string),
        people,
    })
}

/// 交回的 `people`：字的列表，空的、不是字的不要，去重，最多 [`PEOPLE`] 个；不是列表的当没写。
fn people(value: &Value) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for name in value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let name = name.trim();
        if !name.is_empty() && !found.iter().any(|seen| seen == name) && found.len() < PEOPLE {
            found.push(name.to_string());
        }
    }
    found
}

/// 是不是 `YYYY-MM-DD` 的样子：只看字符，不查日子在不在。
fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(at, byte)| match at {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}
