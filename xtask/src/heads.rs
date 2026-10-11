//! `heads.toml`（设计 `32-仓库拆分.md` 第六节，施工 S-5）：两个界面的仓库和取哪一份。发版照它取界面的发布产物，这里只查
//! 写法：正好 `tui`、`web` 两张表，每张只有 `repo`、`rev`；仓库在 `miyu-agent` 组织下；`rev` 是完整的提交号或者 `v` 开头的
//! 标签。

use std::path::Path;

use toml_edit::{DocumentMut, Item};

/// 相对仓库根。
pub const PATH: &str = "heads.toml";

/// 有哪几个界面，照这个先后。
const HEADS: [&str; 2] = ["tui", "web"];

/// 仓库要在这下面。
const ORG: &str = "https://github.com/miyu-agent/";

/// 查 `root` 下的 `heads.toml`，交回问题，没有问题的是空的。
pub fn check(root: &Path) -> Vec<String> {
    match std::fs::read_to_string(root.join(PATH)) {
        Ok(text) => problems(&text),
        Err(error) => vec![format!("{PATH}：读不了（{error}）")],
    }
}

/// 照文件的内容查。
fn problems(text: &str) -> Vec<String> {
    let doc: DocumentMut = match text.parse() {
        Ok(doc) => doc,
        Err(error) => return vec![format!("{PATH}：不是 TOML（{error}）")],
    };
    let mut problems = Vec::new();
    for (key, _) in doc.iter() {
        if !HEADS.contains(&key) {
            problems.push(format!("{PATH}：不认识的表 [{key}]"));
        }
    }
    for head in HEADS {
        let Some(table) = doc.get(head).and_then(Item::as_table) else {
            problems.push(format!("{PATH}：少了表 [{head}]"));
            continue;
        };
        for (key, _) in table.iter() {
            if key != "repo" && key != "rev" {
                problems.push(format!("{PATH}：[{head}] 里不认识的键 {key}"));
            }
        }
        match table.get("repo").and_then(Item::as_str) {
            Some(repo) if repo.starts_with(ORG) && repo.len() > ORG.len() => {}
            Some(repo) => problems.push(format!(
                "{PATH}：[{head}] repo 要在 {ORG} 下，写的是 {repo}"
            )),
            None => problems.push(format!("{PATH}：[{head}] 少了 repo")),
        }
        match table.get("rev").and_then(Item::as_str) {
            Some(rev) if pinned(rev) => {}
            Some(rev) => problems.push(format!(
                "{PATH}：[{head}] rev 要是完整的提交号（40 位小写十六进制）或者 v 开头的标签，写的是 {rev}"
            )),
            None => problems.push(format!("{PATH}：[{head}] 少了 rev")),
        }
    }
    problems
}

/// 钉死了一份：完整的提交号，或者 `v` 开头、后面是数字开头的标签。
fn pinned(rev: &str) -> bool {
    let commit = rev.len() == 40
        && rev
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let tag = rev.strip_prefix('v').is_some_and(|rest| {
        rest.starts_with(|c: char| c.is_ascii_digit()) && !rest.contains(char::is_whitespace)
    });
    commit || tag
}

#[cfg(test)]
mod tests;
