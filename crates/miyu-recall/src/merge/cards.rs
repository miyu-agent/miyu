//! 合并交回来的人卡怎么读（施工 E-1 下，设计 42 第三节第 2 条、PN3、PN4）：交回的对象里多一格
//! `"cards": [{"who", "text", "names"}]`，一个人一张。纯逻辑：调的一方把这一批里出现过的人（`people`，照是谁）交进来。
//!
//! - 只收这一批里出现过的人的卡：别人的卡这一次没看到他的记忆，不许改（同摘要只照交进去的写）。只有名字的人，写成
//!   `person:老王` 这样前面多一截的也认（2026-10-23 协调者定：hy3 这样写了三回，不认的话这个人永远合不成卡）。
//! - 照规矩取：从第一个 `{` 到最后一个 `}`；读不成对象的、`cards` 写了却不是列表的整次算失败（同 [`super::decisions`]）；没写的
//!   当空的。
//! - 一张不合的丢掉、别的照收：是谁不在这一批里、正文空的；同一个人写了两张的照头一张。正文去掉前后空白，超过字数的截在最后
//!   一个句子的结尾上（同摘要）。叫法去掉前后空白，空的、太长的、重复的不要，最多 [`NAMES`] 个。

use std::collections::BTreeSet;

use serde_json::Value;

use super::cut;

/// 一张卡最多几个字（设计 42 PN4，2026-10-21 项目主人定）：让她知道对面是谁、怎么相处，不是全部档案。
pub const CARD_CHARS: usize = 300;

/// 一张卡最多记几个叫法。
pub const NAMES: usize = 8;

/// 一个叫法最多几个字：群名片、外号，长的不是名字。
pub const NAME_CHARS: usize = 40;

/// 交回来的一张卡，照规矩取过的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drafted {
    /// 是谁：一定是这一批里出现过的人。
    pub who: String,
    /// 正文：去掉了前后空白，最多 [`CARD_CHARS`] 个字。
    pub text: String,
    /// 交回的叫法，照先后。
    pub names: Vec<String>,
}

/// 读交回来的字里的人卡：这一批里出现过的人是 `people`。
///
/// # Errors
///
/// 找不到 `{…}`、读不成 JSON 对象、`cards` 写了却不是列表：英文的一句为什么。
pub fn cards(answer: &str, people: &BTreeSet<String>) -> Result<Vec<Drafted>, String> {
    let (Some(start), Some(end)) = (answer.find('{'), answer.rfind('}')) else {
        return Err("no JSON object in the answer".to_string());
    };
    if end < start {
        return Err("no JSON object in the answer".to_string());
    }
    let value: Value = serde_json::from_str(&answer[start..=end])
        .map_err(|error| format!("answer not readable: {error}"))?;
    let items = match &value["cards"] {
        Value::Null => return Ok(Vec::new()),
        Value::Array(items) => items,
        _ => return Err("cards is not a list".to_string()),
    };
    let mut drafted: Vec<Drafted> = Vec::new();
    for item in items {
        if let Some(card) = card(item, people)
            && !drafted.iter().any(|seen| seen.who == card.who)
        {
            drafted.push(card);
        }
    }
    Ok(drafted)
}

/// 一张：不合的是 `None`。
fn card(item: &Value, people: &BTreeSet<String>) -> Option<Drafted> {
    let who = placed(item["who"].as_str()?.trim(), people)?;
    let text = cut(item["text"].as_str()?.trim(), CARD_CHARS);
    if text.is_empty() {
        return None;
    }
    let mut names: Vec<String> = Vec::new();
    for name in item["names"].as_array().into_iter().flatten() {
        let Some(name) = name.as_str().map(str::trim) else {
            continue;
        };
        if !name.is_empty()
            && name.chars().count() <= NAME_CHARS
            && !names.iter().any(|seen| seen == name)
        {
            names.push(name.to_string());
        }
    }
    names.truncate(NAMES);
    Some(Drafted {
        who: who.to_string(),
        text,
        names,
    })
}

/// 交回的是谁对着这一批里的哪个人：正好是的照它；不是的，去掉开头的 `xxx:` 以后正好是这一批里一个只有名字的人（名字里没有
/// `:`）的当成他（弱模型照给它看的那一行把类拼进去，写成 `person:老王`）；平台编号（`qq:10086`）不这么认，别的都不是。
fn placed<'a>(who: &'a str, people: &BTreeSet<String>) -> Option<&'a str> {
    if people.contains(who) {
        return Some(who);
    }
    let (_, name) = who.split_once(':')?;
    let name = name.trim();
    (!name.contains(':') && people.contains(name)).then_some(name)
}

/// 新的一张卡的叫法：交回的在前，再接 `known`（照是谁认出来的那几个、旧卡上的），去重，最多 [`NAMES`] 个。是谁本身不算叫法。
#[must_use]
pub fn names(who: &str, drafted: &[String], known: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in drafted.iter().chain(known) {
        if name != who && !names.contains(name) {
            names.push(name.clone());
        }
    }
    names.truncate(NAMES);
    names
}
