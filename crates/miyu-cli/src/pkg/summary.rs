//! 从软件源装之前印的那一份、问的那两句（施工 IN-3，设计 `49-装包照pacman.md` 第二节第 2、3 条，照 pacman）：核心看一眼的回应
//! （`package.install` 带 `preview`，带 `download` 的就是从软件源装）印成「软件包 (N)」「推荐 (N)」两行、空一行，再几行「键 : 值」
//! （对齐同 `info`）。都是纯的，测试里照它算。

use serde_json::Value;

use super::info::{aligned, capabilities};
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
/// （核心没给的不印）、来源、需要的能力（没有的不印）几行「键 : 值」。
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
    let official = preview["official"].as_bool() == Some(true);
    let source = preview["source"].as_str().unwrap_or_default();
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
            (
                InfoKey::Origin,
                Some(language.verified_source(official, source)),
            ),
            (InfoKey::Capabilities, capabilities(preview, language)),
        ],
        language,
    ));
    rows
}

/// 问的那一句：一个包的「安装 '<编号>'？」，几个的「安装 N 个软件包？」。
pub(super) fn question(preview: &Value, language: &Language) -> String {
    match packages(preview).as_slice() {
        [(id, _)] => language.proceed(Doing::Install, id),
        many => language.proceed_installs(many.len()),
    }
}

#[cfg(test)]
mod tests;
