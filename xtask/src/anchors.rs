//! 参考里的锚点（施工 DC-1）：`docs/reference/` 里写的 `[…](#锚点)`、`[…](别的页.md#锚点)`，那一页要有照这个锚点的标题。
//! 中文的 `connection.md` 目录里一直链着 `#subscribeunsubscribe`，那一节在 SU-1 时误删了，没人看得出来：方法的检查只认
//! `xxx.yyy` 这样带点的名字，`subscribe` 不带点。
//!
//! 锚点照 GitHub 的算法：标题的字转小写，字母、数字（连中文）、`-`、`_` 留下，空格换成 `-`，别的去掉；同一页同样的标题第二个起
//! 加 `-1`、`-2`。代码块里的 `#` 开头的行不是标题。链到 `docs/reference/` 以外的不查。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::reference::PATH;

/// 查参考里的锚点，交回问题。
pub fn check(root: &Path) -> Vec<String> {
    let dir = root.join(PATH);
    let mut pages = BTreeMap::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) => return vec![format!("读不了 {PATH}：{e}")],
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".md") {
            continue;
        }
        match std::fs::read_to_string(entry.path()) {
            Ok(text) => pages.insert(name, text),
            Err(e) => return vec![format!("读不了 {PATH}/{name}：{e}")],
        };
    }
    problems(&pages)
}

/// 每一页（名字到内容）里断了的锚点。
fn problems(pages: &BTreeMap<String, String>) -> Vec<String> {
    let anchors: BTreeMap<&str, BTreeSet<String>> = pages
        .iter()
        .map(|(name, text)| (name.as_str(), headings(text)))
        .collect();
    let mut problems = Vec::new();
    for (name, text) in pages {
        for (page, anchor) in links(text) {
            let page = if page.is_empty() { name.as_str() } else { page };
            let Some(found) = anchors.get(page) else {
                continue;
            };
            if !found.contains(anchor) {
                problems.push(format!(
                    "{PATH}/{name}：链到 {page}#{anchor}，那一页没有这个标题"
                ));
            }
        }
    }
    problems
}

/// 一页里全部标题的锚点。
fn headings(text: &str) -> BTreeSet<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut anchors = BTreeSet::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        let Some(title) = (!fenced).then(|| title(line)).flatten() else {
            continue;
        };
        let anchor = slug(title);
        let times = seen.entry(anchor.clone()).or_default();
        anchors.insert(match *times {
            0 => anchor,
            n => format!("{anchor}-{n}"),
        });
        *times += 1;
    }
    anchors
}

/// `#` 到 `######` 加一个空格开头的行：交回标题的字。
fn title(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches('#');
    let level = line.len() - rest.len();
    ((1..=6).contains(&level))
        .then(|| rest.strip_prefix(' '))
        .flatten()
}

/// 照 GitHub 的算法算一个标题的锚点。
fn slug(title: &str) -> String {
    title
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// 一页里带锚点的链接：（链到哪一页，本页的是空的；锚点）。网址、别的目录的页不在参考里，查的时候自然跳过。
fn links(text: &str) -> Vec<(&str, &str)> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("](") {
        rest = &rest[at + 2..];
        let end = rest.find([')', ' ']).unwrap_or(rest.len());
        let target = &rest[..end];
        if let Some((page, anchor)) = target.split_once('#')
            && !anchor.is_empty()
        {
            found.push((page, anchor));
        }
    }
    found
}

#[cfg(test)]
mod tests;
