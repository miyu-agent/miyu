//! 从软件源装之前印的那一份、问的那两句（施工 IN-3，设计 `49-装包照pacman.md` 第二节第 2、3 条，照 pacman）：核心看一眼的回应
//! （`package.install` 带 `preview`，带 `download` 的就是从软件源装）印成「软件包 (N)」「推荐 (N)」两行、空一行，再几行「键 : 值」
//! （对齐同 `info`）。都是纯的，测试里照它算。

use serde_json::Value;

use super::approve;
use super::info::aligned;
use super::query::size;
use crate::config::columns;
use crate::language::{Doing, InfoKey, Language};

/// 看一眼的回应是不是从软件源装的：写了要下多大。
pub(super) fn from_source(preview: &Value) -> bool {
    preview["download"].is_u64()
}

/// 这一次要装的，照装的先后（依赖在前，要装的那个最后）：编号、版本。
pub(super) fn packages(preview: &Value) -> Vec<(String, String)> {
    let one = |item: &Value| {
        (
            item["package"].as_str().unwrap_or_default().to_string(),
            item["version"].as_str().unwrap_or_default().to_string(),
        )
    };
    let with = preview["with"].as_array().into_iter().flatten().map(one);
    with.chain(std::iter::once(one(preview))).collect()
}

/// 推荐的、还没装的：编号、版本。
pub(super) fn recommends(preview: &Value) -> Vec<(String, String)> {
    preview["recommends"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            (
                item["package"].as_str().unwrap_or_default().to_string(),
                item["version"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// 「<编号>-<版本>」，没写版本的只有编号。
pub(super) fn named((id, version): &(String, String)) -> String {
    match version.is_empty() {
        true => id.clone(),
        false => format!("{id}-{version}"),
    }
}

/// 印的那一份：「软件包 (N)」一行、有推荐的「推荐 (N)」一行（两行的开头照宽的那个补齐，再空两格），空一行，下载大小、安装后大小
/// （核心没给的不印）、来源、需要的能力（没有的不印）几行「键 : 值」。施工 PK-3 下起（设计 61 第三节）：来源照每个包说，都一样
/// 的只写一行，不一样的一个包一行、前面接编号；能力一项一行、名字接说明，几个包的前面接编号，批过的标出来。
pub(super) fn plan(preview: &Value, language: &Language) -> Vec<String> {
    let installing = packages(preview);
    let recommended = recommends(preview);
    let mut heads = vec![(language.packages_count(installing.len()), &installing)];
    if !recommended.is_empty() {
        heads.push((language.recommended_count(recommended.len()), &recommended));
    }
    let width = heads
        .iter()
        .map(|(head, _)| columns(head))
        .max()
        .unwrap_or(0);
    let mut rows: Vec<String> = heads
        .iter()
        .map(|(head, items)| {
            let items: Vec<String> = items.iter().map(named).collect();
            let pad = " ".repeat(width - columns(head) + 2);
            format!("{head}{pad}{}", items.join("  "))
        })
        .collect();
    rows.push(String::new());
    let items = approve::installing(preview);
    let ids = (items.len() > 1).then(|| id_width(&items));
    let needs = approve::needs_of(&items, ids, language);
    rows.extend(aligned(
        vec![
            (
                InfoKey::DownloadSize,
                preview["download"].as_u64().map(size),
            ),
            (
                InfoKey::InstalledSize,
                preview["installed"].as_u64().map(size),
            ),
            (InfoKey::Origin, Some(origins(preview, &items, language))),
            (InfoKey::Capabilities, joined(needs)),
        ],
        language,
    ));
    rows
}

/// 几个包的编号里最宽的那个占几列。
fn id_width(items: &[&Value]) -> usize {
    items
        .iter()
        .map(|item| columns(item["package"].as_str().unwrap_or_default()))
        .max()
        .unwrap_or(0)
}

/// 几行连成一个值（`aligned` 照换行缩进），一行都没有的没有。
fn joined(lines: Vec<String>) -> Option<String> {
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// 「来源」：每个包照它的 `source`、`feed` 说（施工 PK-3 下）；都一样的只写一行，不一样的一个包一行、前面接编号。旧的核心
/// （施工 PK-3 上以前，没有 `feed`，`source` 是源的名字）照旧那一句。
fn origins(preview: &Value, items: &[&Value], language: &Language) -> String {
    if !preview["feed"].is_string() {
        let official = preview["official"].as_bool() == Some(true);
        let source = preview["source"].as_str().unwrap_or_default();
        return language.verified_source(official, source);
    }
    let said: Vec<(&str, String)> = items
        .iter()
        .filter_map(|item| {
            let feed = item["feed"].as_str()?;
            let source = item["source"].as_str().unwrap_or_default();
            let line = language.verified_from(source, feed, item["origin"].as_str());
            Some((item["package"].as_str().unwrap_or_default(), line))
        })
        .collect();
    if said.iter().all(|(_, line)| *line == said[0].1) {
        return said[0].1.clone();
    }
    let width = Some(id_width(items));
    let lines: Vec<String> = said
        .iter()
        .map(|(id, line)| approve::prefixed(id, width, line))
        .collect();
    lines.join("\n")
}

/// 问的那一句：一个包的「安装 '<编号>'？」，几个的「安装 N 个软件包？」；有要批的能力的（核心认 `approve` 的）换成「并批准
/// 它（们）要的能力」（施工 PK-3 下）。核心给了 `notice` 的，前面先一行「注意：…」。
pub(super) fn question(preview: &Value, language: &Language) -> String {
    let approving = approve::understood(preview) && approve::asks(&approve::installing(preview));
    let asked = match (packages(preview).as_slice(), approving) {
        ([(id, _)], false) => language.proceed(Doing::Install, id),
        ([(id, _)], true) => language.proceed_approving(Doing::Install, id),
        (many, false) => language.proceed_installs(many.len()),
        (many, true) => language.proceed_many_approving(Doing::Install, many.len()),
    };
    approve::noticed(approve::notice(&preview["notice"], language), asked)
}

/// 推荐的那一句（施工 PK-3 下，设计 61 第三节「推荐的」）：推荐的里有要能力的、核心给了 `recommends_notice` 的，先空一行另印
/// 「推荐的包要的能力」（一项一行、前面接编号）、「注意：…」，问的换成「并批准」；都没有的照旧那一句。
pub(super) fn recommended_question(preview: &Value, language: &Language) -> String {
    let items: Vec<&Value> = preview["recommends"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let width = Some(id_width(&items));
    let needs = approve::needs_of(&items, width, language);
    let notice = approve::notice(&preview["recommends_notice"], language);
    let asked = match approve::understood(preview) && approve::asks(&items) {
        true => language.proceed_recommended_approving(items.len()),
        false => language.proceed_recommended(items.len()),
    };
    if needs.is_empty() && notice.is_none() {
        return asked;
    }
    let mut said = vec![String::new()];
    said.extend(aligned(
        vec![(InfoKey::RecommendedCapabilities, joined(needs))],
        language,
    ));
    said.extend(notice);
    said.push(asked);
    said.join("\n")
}

#[cfg(test)]
mod tests;
