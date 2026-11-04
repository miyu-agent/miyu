//! 导出前最后过一遍（调研第五节第 7 条）：配了的密钥原样出现的换成 `[redacted: <名字>]`；家目录写成 `~`。
//!
//! - 密钥只认一模一样的值：精确匹配，不猜格式。太短的（不到 [`SHORTEST`] 个字）不换：短值在正文里撞上的多半不是它，换了
//!   反倒把正文弄坏。几个密钥的值互相包含、重叠的，从左往右取，同一处起头的取最长的那一个，换过的字不再看。
//! - 家目录只换成整段路径的：前后都不是路径里的字（`/home/alice` 换，`/home/alicex`、`/x/home/alice` 不换）。

use std::collections::BTreeMap;

/// 密钥的值至少几个字才换（照 Unicode 的字数）。
const SHORTEST: usize = 8;

/// 把 `text` 里配了的密钥（名字、值）换掉；交回换好的和每个名字换了几处（一处都没有的不列）。
pub(super) fn redact(text: &str, secrets: &[(&str, &str)]) -> (String, BTreeMap<String, usize>) {
    let mut found: Vec<(usize, usize, &str)> = Vec::new();
    for (name, value) in secrets {
        if value.chars().count() < SHORTEST {
            continue;
        }
        found.extend(
            text.match_indices(value)
                .map(|(at, value)| (at, at + value.len(), *name)),
        );
    }
    // 起头早的先；同一处起头的长的先。
    found.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut out = String::with_capacity(text.len());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut done = 0;
    for (start, end, name) in found {
        if start < done {
            continue;
        }
        out.push_str(&text[done..start]);
        out.push_str(&format!("[redacted: {name}]"));
        *counts.entry(name.to_string()).or_default() += 1;
        done = end;
    }
    out.push_str(&text[done..]);
    (out, counts)
}

/// 把 `text` 里的家目录 `home` 写成 `~`。没有家目录的、家目录是空的、是根的原样交回。
pub(super) fn tilde(text: &str, home: Option<&str>) -> String {
    let Some(home) = home
        .map(|home| home.trim_end_matches(['/', '\\']))
        .filter(|home| !home.is_empty())
    else {
        return text.to_string();
    };
    let mut out = String::with_capacity(text.len());
    let mut done = 0;
    for (at, _) in text.match_indices(home) {
        if at < done {
            continue;
        }
        let before = text[..at].chars().next_back();
        let after = text[at + home.len()..].chars().next();
        let inside = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || "_-.".contains(c));
        if inside(before) || before.is_some_and(|c| c == '/' || c == '\\') || inside(after) {
            continue;
        }
        out.push_str(&text[done..at]);
        out.push('~');
        done = at + home.len();
    }
    out.push_str(&text[done..]);
    out
}
