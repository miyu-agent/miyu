//! 人经协议把别处的记忆导进来（`memory.import`，施工 MI-1，`docs/blueprint/memory.md`「协议」）：旧版的迁移包照它把知识点
//! 导成新的记忆，保住原来记下的时刻。一批先全查，一条不对整批不写，`bad_params` 的 `data` 说是第几条、哪一项；查过的交给
//! [`Keeper::import`] 照 `origin` 去重、记下，导进了的在后台写关键词、补向量、照 E-9 看一眼够不够整理。
//!
//! [`Keeper::import`]: miyu_session::Keeper::import

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use miyu_kernel::time::Timestamp;
use miyu_recall::{CLASSES, EPISODE, KEYWORD_CHARS, MemoryId, PERSON};
use miyu_session::{Imported, Stamp};
use miyu_tool::TEXT_CHARS;

use super::params::Import;
use super::{blocking, find, using};
use crate::Core;
use crate::refusal::Refusal;
use crate::sessions::{now, offset};

/// 一批最多几条。
pub(super) const ITEMS: usize = 500;
/// `origin` 最多几个字。
const ORIGIN_CHARS: usize = 200;

/// `memory.import`：查过整批，记下，交回 `{"ids", "imported", "skipped"}`；`ids` 和 `items` 一一对上，跳过的是 `null`。
pub(super) async fn import(
    core: &Arc<Core>,
    stamp: Stamp,
    params: Import,
) -> Result<Value, Refusal> {
    if params.items.is_empty() || params.items.len() > ITEMS {
        return Err(bad(None, "items"));
    }
    let mut origins = BTreeSet::new();
    let items = params
        .items
        .iter()
        .enumerate()
        .map(|(index, raw)| item(index, raw, &mut origins))
        .collect::<Result<Vec<Imported>, Refusal>>()?;
    let keeper = find(core, params.at).await?;
    let ids = blocking({
        let keeper = keeper.clone();
        move || keeper.import(stamp, items)
    })
    .await?;
    let saved: Vec<MemoryId> = ids.iter().flatten().copied().collect();
    let skipped = ids.len() - saved.len();
    let shown: Vec<Value> = ids
        .iter()
        .map(|id| id.map_or(Value::Null, |id| Value::String(id.to_string())))
        .collect();
    let imported = saved.len();
    keeper.settle_imported(saved, using(core).config, offset());
    Ok(json!({"ids": shown, "imported": imported, "skipped": skipped}))
}

/// 查第 `index` 条：`at`、`class`、`text`、`origin` 必写，`who` 只有认识的人那一类写、而且必写，`people` 只有发生过的事
/// 那一类能写，`keywords` 可以不写。`origin` 一批里不能重复（`seen` 记着前面的）。
fn item(index: usize, raw: &Value, seen: &mut BTreeSet<String>) -> Result<Imported, Refusal> {
    let bad = |field| bad(Some(index), field);
    let raw = raw.as_object().ok_or_else(|| bad("item"))?;
    let at = text(raw, "at")
        .and_then(|at| at.parse::<jiff::Timestamp>().ok())
        .and_then(|at| Timestamp::from_unix_millis(at.as_millisecond()))
        .filter(|at| *at <= now())
        .ok_or_else(|| bad("at"))?;
    let class = text(raw, "class")
        .filter(|class| CLASSES.contains(class))
        .ok_or_else(|| bad("class"))?
        .to_string();
    let words = text(raw, "text")
        .map(str::trim)
        .filter(|words| !words.is_empty() && words.chars().count() <= TEXT_CHARS)
        .ok_or_else(|| bad("text"))?
        .to_string();
    let origin = text(raw, "origin")
        .map(str::trim)
        .filter(|origin| !origin.is_empty() && origin.chars().count() <= ORIGIN_CHARS)
        .filter(|origin| seen.insert((*origin).to_string()))
        .ok_or_else(|| bad("origin"))?
        .to_string();
    let who = match (class == PERSON, raw.get("who")) {
        (true, Some(Value::String(who))) if !who.trim().is_empty() => Some(who.trim().to_string()),
        (false, None | Some(Value::Null)) => None,
        _ => return Err(bad("who")),
    };
    let people = match (class == EPISODE, raw.get("people")) {
        (_, None | Some(Value::Null)) => Vec::new(),
        (true, Some(Value::Array(people))) => people
            .iter()
            .map(|one| {
                one.as_str()
                    .map(str::trim)
                    .filter(|one| !one.is_empty())
                    .map(str::to_string)
            })
            .collect::<Option<Vec<String>>>()
            .ok_or_else(|| bad("people"))?,
        _ => return Err(bad("people")),
    };
    let keywords = match raw.get("keywords") {
        None | Some(Value::Null) => None,
        Some(Value::String(keywords)) if keywords.trim().chars().count() <= KEYWORD_CHARS => {
            Some(keywords.trim().to_string()).filter(|keywords| !keywords.is_empty())
        }
        Some(_) => return Err(bad("keywords")),
    };
    Ok(Imported {
        at,
        class,
        text: words,
        origin,
        who,
        people,
        keywords,
    })
}

/// 一项的字：没写、不是字符串的没有。
fn text<'a>(raw: &'a Map<String, Value>, field: &str) -> Option<&'a str> {
    raw.get(field).and_then(Value::as_str)
}

/// 参数不对：`data` 写第几条（整批的毛病没有）、哪一项。
fn bad(index: Option<usize>, field: &str) -> Refusal {
    let mut data = Map::new();
    if let Some(index) = index {
        data.insert("index".to_string(), json!(index));
    }
    data.insert("field".to_string(), json!(field));
    Refusal {
        data: Some(data),
        ..Refusal::BAD_PARAMS
    }
}
