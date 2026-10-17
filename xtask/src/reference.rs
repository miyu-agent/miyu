//! 协议参考（`docs/reference/`，施工 A-1）：代码里分发的、核心发给扩展的每一个方法，中文、英文两份参考里都要有条目；参考里
//! 当标题写的方法，代码里要有。算在「文档」那一项里。
//!
//! 代码这一头照字面找，不跑核心：端点和核心的非测试代码里，样子是 `xxx.yyy` 的字符串字面量，出现在分发的位置——`match` 的
//! 分支（`"…" =>`、`| "…"`）、元组的分支和调用的第一个参数（`("…",`）、`"method": "…"`、`{ "…" }`——和登记查询的包里一行
//! 一个的列表。参考这一头认 `## ` 标题和表格第一格里写的。

use std::collections::BTreeSet;
use std::path::Path;

/// 参考在仓库里的位置。
pub const PATH: &str = "docs/reference";

/// 找方法的代码目录。
const SOURCES: [&str; 2] = ["crates/miyu-endpoint/src", "crates/miyu-core/src"];

/// 查参考，交回问题。
pub fn check(root: &Path) -> Vec<String> {
    let mut code = BTreeSet::new();
    for dir in SOURCES {
        if let Err(e) = scan_dir(&root.join(dir), &mut code) {
            return vec![format!("读不了 {dir}：{e}")];
        }
    }
    let (zh, en, headings) = match documented(&root.join(PATH)) {
        Ok(found) => found,
        Err(e) => return vec![format!("读不了 {PATH}：{e}")],
    };
    let mut problems = Vec::new();
    for name in &code {
        if !zh.contains(name) {
            problems.push(format!("{PATH}：代码里有 {name}，中文参考里没有它的条目"));
        }
        if !en.contains(name) {
            problems.push(format!(
                "{PATH}：代码里有 {name}，英文参考（*.en.md）里没有它的条目"
            ));
        }
    }
    for (name, file) in headings {
        if !code.contains(&name) {
            problems.push(format!("{PATH}/{file}：标题里的 {name} 代码里没有"));
        }
    }
    problems
}

/// 读一个目录下的 `.rs`，测试的文件、目录跳过。
fn scan_dir(dir: &Path, found: &mut BTreeSet<String>) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                scan_dir(&path, found)?;
            }
        } else if name.ends_with(".rs")
            && !matches!(name.as_str(), "tests.rs" | "test_support.rs" | "testkit.rs")
        {
            scan(&std::fs::read_to_string(&path)?, found);
        }
    }
    Ok(())
}

/// 一份代码里分发的方法名。
pub fn scan(text: &str, found: &mut BTreeSet<String>) {
    let text = text
        .split("#[cfg(test)]\nmod tests {")
        .next()
        .unwrap_or_default();
    let listing = text.contains("register");
    let mut rest = text;
    let mut offset = 0;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        let literal = &after[..end];
        let at = offset + start;
        let close = at + 1 + end + 1;
        if is_method(literal) && dispatched(text, at, close, listing) {
            found.insert(literal.to_string());
        }
        offset = close;
        rest = &text[close..];
    }
}

/// `xxx.yyy`：小写字母、`_`，至少一个点，点两边不空。
fn is_method(text: &str) -> bool {
    text.contains('.')
        && text.split('.').all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        })
}

/// 从 `at`（开头的引号）到 `close`（结尾引号后面）的字面量是不是在分发的位置。
fn dispatched(text: &str, at: usize, close: usize, listing: bool) -> bool {
    let before = text[..at].trim_end();
    let after = text[close..].trim_start();
    let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
    let alone = text[line_start..at].trim().is_empty();
    after.starts_with("=>")
        || (after.starts_with('|') && !after.starts_with("||"))
        || (before.ends_with('|') && !before.ends_with("||"))
        || (before.ends_with('(') && after.starts_with(','))
        || (before.ends_with('{') && after.starts_with('}'))
        || before.ends_with("\"method\":")
        || (listing && alone && after.starts_with(",\n"))
}

/// 参考里写了条目的方法：中文的、英文的，和标题里写的（方法、文件）。
#[allow(clippy::type_complexity)]
fn documented(
    dir: &Path,
) -> std::io::Result<(
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<(String, String)>,
)> {
    let (mut zh, mut en, mut headings) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut files: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    files.sort_by_key(std::fs::DirEntry::file_name);
    for file in files {
        let name = file.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".md") {
            continue;
        }
        let text = std::fs::read_to_string(file.path())?;
        let set = if name.ends_with(".en.md") {
            &mut en
        } else {
            &mut zh
        };
        for line in text.lines() {
            let heading = line.starts_with("## ");
            if !(heading || line.starts_with("| `") || line.starts_with("| [`")) {
                continue;
            }
            let cell = if heading {
                line
            } else {
                line.split(" |").next().unwrap_or(line)
            };
            for method in quoted(cell) {
                if heading {
                    headings.insert((method.clone(), name.clone()));
                }
                set.insert(method);
            }
        }
    }
    Ok((zh, en, headings))
}

/// 一行里反引号括起来、样子是方法名的。
fn quoted(line: &str) -> Vec<String> {
    line.split('`')
        .skip(1)
        .step_by(2)
        .filter(|part| is_method(part))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests;
