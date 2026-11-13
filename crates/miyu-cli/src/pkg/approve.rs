//! 装、升级的时候就批能力（施工 PK-3 下，设计 `61-装包说清来源和能力.md` 第三节、第四节第 2 条）：确认屏上能力一项一行、「注意」
//! 那一行、问的那一句换不换成「并批准」；答「是」（`--yes` 也算，定了的 6）的照看一眼列出来的每个包的能力编号原样拼成请求的
//! `approve`；装好、升好以后一个包一行说批了哪些。都是纯的，测试里照它算。

use serde_json::{Map, Value, json};

use crate::config::columns;
use crate::language::Language;

/// 核心认不认 `approve`：施工 PK-3 上起才认，以前的核心参数是 `deny_unknown_fields`，带了整条请求被拒。认的核心看一眼写
/// `feed`（从软件源装的都写）或者 `source: "file"`（从包文件夹、包文件装的）；以前从软件源装的 `source` 是源的名字、没有
/// `feed`，从包文件夹装的没有 `source`。
pub(super) fn understood(preview: &Value) -> bool {
    preview["feed"].is_string() || preview["source"] == "file"
}

/// 一项（顶上的、`with`、`recommends`、`upgrades` 的一项）列出来的能力。
fn listed(item: &Value) -> impl Iterator<Item = &Value> {
    item["capabilities"].as_array().into_iter().flatten()
}

/// 这几项里有没有要批的：列了、又不是 `held` 的。
pub(super) fn asks(items: &[&Value]) -> bool {
    items
        .iter()
        .any(|item| listed(item).any(|one| one["held"] != true))
}

/// 装的这一次的几项：一起装的依赖（`with`）在前，要装的那个最后，同「软件包 (N)」那一行。
pub(super) fn installing(preview: &Value) -> Vec<&Value> {
    let with = preview["with"].as_array().into_iter().flatten();
    with.chain(std::iter::once(preview)).collect()
}

/// 升级的这一次的几项：升的（`upgrades`），再一起装的新依赖（`with`）。
pub(super) fn upgrading(preview: &Value) -> Vec<&Value> {
    let upgrades = preview["upgrades"].as_array().into_iter().flatten();
    upgrades
        .chain(preview["with"].as_array().into_iter().flatten())
        .collect()
}

/// 照几项拼 `approve`：每一项列出来的能力编号照原样带上（`held` 的也带：核心照并集记，不会多批）；没列能力的不写。
pub(super) fn table(items: &[&Value]) -> Map<String, Value> {
    let mut approve = Map::new();
    for item in items {
        let ids: Vec<&str> = listed(item).filter_map(|one| one["id"].as_str()).collect();
        if let (Some(id), false) = (item["package"].as_str(), ids.is_empty()) {
            approve.insert(id.to_string(), json!(ids));
        }
    }
    approve
}

/// 把 `approve` 带进请求 `params`：空的不带（没有要批的，也照顾不认它的核心）。
pub(super) fn carried(mut params: Value, approve: &Map<String, Value>) -> Value {
    if !approve.is_empty() {
        params["approve"] = Value::Object(approve.clone());
    }
    params
}

/// 批了哪些：一个包一行「已批准 <编号> 的能力：<名字>、…」，只列这一次新批的（不是 `held` 的），没有的不印。
pub(super) fn said(items: &[&Value], language: &Language) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| {
            let names: Vec<&str> = listed(item)
                .filter(|one| one["held"] != true)
                .filter_map(|one| one["name"].as_str())
                .collect();
            let id = item["package"].as_str()?;
            (!names.is_empty())
                .then(|| language.approved(id, &names.join(language.list_separator())))
        })
        .collect()
}

/// 一项的能力一项一行：核心给的名字接说明，`held` 的标「已批准」；`fresh` 的只列不是 `held` 的（升级的「新要」）。
pub(super) fn needs(item: &Value, fresh: bool, language: &Language) -> Vec<String> {
    listed(item)
        .filter(|one| !(fresh && one["held"] == true))
        .filter_map(|one| {
            let name = one["name"].as_str()?;
            Some(language.capability(name, one["summary"].as_str(), one["held"] == true))
        })
        .collect()
}

/// 几个包的能力，一项一行、前面接编号（编号照 `width` 补齐、再空两格）；只有一个包的（`width` 是 `None`）不接编号。
pub(super) fn needs_of(items: &[&Value], width: Option<usize>, language: &Language) -> Vec<String> {
    items
        .iter()
        .flat_map(|item| {
            let id = item["package"].as_str().unwrap_or_default();
            needs(item, false, language)
                .into_iter()
                .map(move |line| prefixed(id, width, &line))
        })
        .collect()
}

/// `line` 前面接编号 `id`，编号补到 `width` 列再空两格；`width` 是 `None` 的照原样。
pub(super) fn prefixed(id: &str, width: Option<usize>, line: &str) -> String {
    match width {
        Some(width) => {
            let pad = " ".repeat(width.saturating_sub(columns(id)) + 2);
            format!("{id}{pad}{line}")
        }
        None => line.to_string(),
    }
}

/// 「注意：」那一行：核心给的 `notice`（一句一个字符串）照原样接；没有的没有。
pub(super) fn notice(value: &Value, language: &Language) -> Option<String> {
    let sentences: Vec<&str> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    (!sentences.is_empty()).then(|| language.notice(&sentences))
}

/// 问的那一句前面接「注意」那一行（有的话）：空一行以后先印「注意」，紧接着问。
pub(super) fn noticed(notice: Option<String>, question: String) -> String {
    match notice {
        Some(notice) => format!("{notice}\n{question}"),
        None => question,
    }
}

#[cfg(test)]
mod tests;
