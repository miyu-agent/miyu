//! 一条记忆在 `memory.list`、`memory.search` 的回应里的样子（施工 R-3 补，`docs/blueprint/protocol.md` 的 `memory.*` 第 1 条）：
//! 用到几次、最近一次（施工 R-4 下），在哪个场所说的（施工 R-13 上），发生过的事涉及谁（施工 E-1 修四，没有的是空列表），在哪个群（施工 E-4 上，没有的是 `null`）。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use miyu_recall::Entry;
use miyu_store::recall::Used;

/// 几条在回应里的样子，带上用到几次、最近一次（施工 R-4 下）。
pub(super) fn shown_all(found: &[Entry], used: &BTreeMap<String, Used>) -> Vec<Value> {
    found
        .iter()
        .map(|entry| {
            let mut value = shown(entry);
            let used = used.get(&entry.id.to_string());
            value["used"] = json!(used.map_or(0, |used| used.count));
            value["used_at"] = json!(used.map(|used| used.last.to_string()));
            value
        })
        .collect()
}

/// 一条在回应里的样子。
fn shown(entry: &Entry) -> Value {
    let by = serde_json::to_value(&entry.by)
        .ok()
        .and_then(|by| by.get("kind").cloned())
        .unwrap_or(Value::Null);
    json!({
        "id": entry.id.to_string(),
        "class": entry.class,
        "text": entry.text,
        "at": entry.at.to_string(),
        "by": by,
        "sources": entry.sources,
        "retired": entry.retired,
        "venue": entry.venue,
        "keywords": entry.keywords,
        "who": entry.who,
        "people": entry.people,
        "place": entry.place,
    })
}
