//! 合并交回来的怎么读（施工 R-7 上，`docs/blueprint/memory.md` 第七条第 3 款）：模型交回一个 JSON 对象
//! `{"revised": [{"id", "text"}], "retired": [{"id", "kind", "by", "why"}], "summary": "…"}`。纯逻辑：调的一方把这一次交进去的
//! 编号、一条最多几个字、摘要最多几个字交进来。
//!
//! - 作废照规矩核对（施工 R-16，2026-10-19 量：一次交几十条时模型偶尔弄反新旧、把重复的两条都删）：模型说的是申请，核对得上
//!   的才作废，宁可不删。没写 `kind` 的、写了别的的不作废；`outdated`（被后记的取代）、`repeat`（和另一条说的一样）的 `by` 要是
//!   这一次交进去的、不是自己；`outdated` 的 `by` 要比它记得晚（编号大）；`by` 那一条也要作废的，`outdated` 的不作废它，`repeat`
//!   的互相指着时只作废先记的那一条。`worthless`（以后用不着）不看 `by`。
//!
//! - 照规矩取：从第一个 `{` 到最后一个 `}`，前后多说的话不管；读不成 JSON 对象的、`revised`、`retired` 写了却不是列表的、
//!   `summary` 写了却不是字的，整次算失败。没写的列表当空的。
//! - 一条不合的丢掉、别的照收：编号不是这一次交进去的、正文空的或超了字数、`why` 空的；同一条写了两次的照头一次。同一条又改
//!   又作废的照作废（不留一条马上要作废的新版本）。
//! - 摘要：去掉前后空白，空的当没写（不换掉现在的）；超过字数的截在最后一个句号、问号、叹号、换行上，一句都放不下的照字截。

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use serde_json::Value;

use crate::memory::MemoryId;

/// 改一条：编号、新的正文（去掉了前后空白）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revised {
    /// 改哪一条：一定是交进去的那几条之一。
    pub id: MemoryId,
    /// 新的正文。
    pub text: String,
}

/// 作废一条：编号、哪一种、被哪一条取代、为什么（去掉了前后空白，不是空的）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retire {
    /// 作废哪一条：一定是交进去的那几条之一。
    pub id: MemoryId,
    /// 哪一种（施工 R-16）。
    pub kind: Kind,
    /// 取代它、和它重复的那一条：`outdated`、`repeat` 一定有，是交进去的另一条；`worthless` 没有。
    pub by: Option<MemoryId>,
    /// 为什么。
    pub why: String,
}

/// 作废的是哪一种（施工 R-16；R-17 起同一种记进日志）。
pub use crate::memory::RetireKind as Kind;

/// 交回来的、照规矩取过的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decisions {
    /// 要改的，照交回的先后。
    pub revised: Vec<Revised>,
    /// 要作废的，照交回的先后。
    pub retired: Vec<Retire>,
    /// 新的摘要；没写的、空的没有。
    pub summary: Option<String>,
}

/// 读交回来的字：这一次交进去的是 `given` 这几条，一条最多 `max_chars` 个字，摘要最多 `summary_chars` 个字。
///
/// # Errors
///
/// 找不到 `{…}`、读不成 JSON 对象、`revised` 或 `retired` 不是列表、`summary` 不是字：英文的一句为什么。
pub fn decisions(
    answer: &str,
    given: &BTreeSet<MemoryId>,
    max_chars: usize,
    summary_chars: usize,
) -> Result<Decisions, String> {
    let (Some(start), Some(end)) = (answer.find('{'), answer.rfind('}')) else {
        return Err("no JSON object in the answer".to_string());
    };
    if end < start {
        return Err("no JSON object in the answer".to_string());
    }
    let value: Value = serde_json::from_str(&answer[start..=end])
        .map_err(|error| format!("answer not readable: {error}"))?;
    if !value.is_object() {
        return Err("the answer is not a JSON object".to_string());
    }
    let mut retired = Vec::new();
    for item in list(&value, "retired")? {
        if let Some(retire) = retire(item, given)
            && !retired.iter().any(|seen: &Retire| seen.id == retire.id)
        {
            retired.push(retire);
        }
    }
    checked(&mut retired);
    let mut revised = Vec::new();
    for item in list(&value, "revised")? {
        if let Some(revise) = revise(item, given, max_chars)
            && !revised.iter().any(|seen: &Revised| seen.id == revise.id)
            && !retired.iter().any(|gone| gone.id == revise.id)
        {
            revised.push(revise);
        }
    }
    let summary = match &value["summary"] {
        Value::Null => None,
        Value::String(text) => {
            Some(cut(text.trim(), summary_chars)).filter(|text| !text.is_empty())
        }
        _ => return Err("summary is not a string".to_string()),
    };
    Ok(Decisions {
        revised,
        retired,
        summary,
    })
}

/// `name` 那一格的列表：没写的是空的，写了不是列表的出错。
fn list<'a>(value: &'a Value, name: &str) -> Result<&'a [Value], String> {
    match &value[name] {
        Value::Null => Ok(&[]),
        Value::Array(items) => Ok(items),
        _ => Err(format!("{name} is not a list")),
    }
}

/// 交进去的那几条之一的编号：别的是 `None`。
fn given_id(item: &Value, given: &BTreeSet<MemoryId>) -> Option<MemoryId> {
    item["id"]
        .as_str()
        .and_then(MemoryId::parse)
        .filter(|id| given.contains(id))
}

/// 改一条：不合的是 `None`。
fn revise(item: &Value, given: &BTreeSet<MemoryId>, max_chars: usize) -> Option<Revised> {
    let id = given_id(item, given)?;
    let text = item["text"].as_str()?.trim();
    if text.is_empty() || text.chars().count() > max_chars {
        return None;
    }
    Some(Revised {
        id,
        text: text.to_string(),
    })
}

/// 作废一条：不合的是 `None`（没写种类、写了别的；要 `by` 的没写、不是交进去的、指着自己；`outdated` 的 `by` 不比它晚）。
fn retire(item: &Value, given: &BTreeSet<MemoryId>) -> Option<Retire> {
    let id = given_id(item, given)?;
    let why = item["why"].as_str()?.trim();
    if why.is_empty() {
        return None;
    }
    let kind = match item["kind"].as_str()? {
        "outdated" => Kind::Outdated,
        "repeat" => Kind::Repeat,
        "worthless" => Kind::Worthless,
        _ => return None,
    };
    let by = match kind {
        Kind::Worthless => None,
        Kind::Outdated | Kind::Repeat => {
            let by = item["by"]
                .as_str()
                .and_then(MemoryId::parse)
                .filter(|by| given.contains(by) && *by != id)?;
            if kind == Kind::Outdated && by < id {
                return None;
            }
            Some(by)
        }
    };
    Some(Retire {
        id,
        kind,
        by,
        why: why.to_string(),
    })
}

/// `by` 那一条也要作废的（施工 R-16）：`outdated` 的不作废它，宁可留着；`repeat` 的互相指着，只作废先记的那一条（后记的留下）。
/// 照到没有可去掉的为止。
fn checked(retired: &mut Vec<Retire>) {
    loop {
        let gone = |id: MemoryId, retired: &[Retire]| retired.iter().position(|one| one.id == id);
        let mut drop = None;
        for (at, one) in retired.iter().enumerate() {
            let Some(target) = one.by.and_then(|by| gone(by, retired)) else {
                continue;
            };
            let other = &retired[target];
            drop = match one.kind {
                Kind::Outdated => Some(at),
                Kind::Repeat if one.id > other.id => Some(at),
                Kind::Repeat if other.kind == Kind::Repeat && other.by == Some(one.id) => {
                    Some(target)
                }
                _ => None,
            };
            if drop.is_some() {
                break;
            }
        }
        match drop {
            Some(at) => {
                retired.remove(at);
            }
            None => return,
        }
    }
}

/// 最多 `limit` 个字：截在最后一个句子的结尾上，一句都放不下的照字截。
fn cut(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit).collect();
    match head.rfind(['。', '！', '？', '.', '!', '?', '\n']) {
        Some(at) => {
            let end = at + head[at..].chars().next().map_or(0, char::len_utf8);
            head[..end].trim_end().to_string()
        }
        None => head,
    }
}
