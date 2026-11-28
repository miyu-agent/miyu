//! 发行版订阅里的标签（施工 UP-4，`internal/blueprint/cli/upgrade.md`「怎么走」第 4 条）：`https://github.com/<仓库>/releases.atom`
//! 是普通网页，不算 GitHub API 的匿名额度。草稿不在里面；没有「预发布」的标记，预发布照标签认（[`super::plan::candidates`]）。
//!
//! 不为它加 XML 的依赖：只认每个 `<entry>` 里第一个 `href` 带 `/releases/tag/` 的 `<link>`。正文（`<content>`）是转义过的网页，
//! 里面的 `<`、`"` 都写成了 `&lt;`、`&quot;`，不会被当成元素。

/// 订阅里每个 entry 的标签，照订阅的先后。不是 UTF-8、没有 `<feed`、没有 `</feed>`（空的、网页、读到一半的）的不是 Atom 订阅，
/// 没有；是订阅、一个 entry 都没有的是空的。
pub(crate) fn tags(bytes: &[u8]) -> Option<Vec<String>> {
    let text = std::str::from_utf8(bytes).ok()?;
    if !text.contains("<feed") || !text.contains("</feed>") {
        return None;
    }
    let mut tags = Vec::new();
    let mut rest = text;
    while let Some(start) = next_entry(rest) {
        let entry = &rest[start..];
        let end = entry.find("</entry>").unwrap_or(entry.len());
        if let Some(tag) = tag_of(&entry[..end]) {
            tags.push(tag.to_string());
        }
        rest = &entry[end..];
    }
    Some(tags)
}

/// 下一个 `<entry>`（或者带属性的 `<entry …>`）在哪；`<entries` 这样的别的元素不算。
fn next_entry(text: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(found) = text[from..].find("<entry") {
        let at = from + found;
        let after = text[at + "<entry".len()..].chars().next();
        if matches!(after, Some('>' | ' ' | '\t' | '\r' | '\n')) {
            return Some(at + "<entry".len());
        }
        from = at + "<entry".len();
    }
    None
}

/// 一个 entry 里 `releases/tag/<标签>` 链接的标签：第一个 `href` 带 `/releases/tag/` 的 `<link>`。
fn tag_of(entry: &str) -> Option<&str> {
    let mut rest = entry;
    while let Some(start) = rest.find("<link") {
        let element = &rest[start..];
        let end = element.find('>').unwrap_or(element.len());
        let link = &element[..end];
        let href = link
            .split_once("href=\"")
            .and_then(|(_, value)| value.split_once('"'))
            .map(|(value, _)| value);
        if let Some(tag) = href.and_then(|href| href.split_once("/releases/tag/")) {
            let tag = tag.1;
            if !tag.is_empty() && !tag.contains('/') {
                return Some(tag);
            }
        }
        rest = &element[end..];
    }
    None
}

#[cfg(test)]
mod tests;
