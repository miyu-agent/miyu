//! `heads.toml`（设计 `32-仓库拆分.md` 第六节，施工 S-5）：两个界面的仓库和取哪一份。发版照它取界面的发布产物，这里只查
//! 写法：正好 `miyu-tui`、`miyu-webui` 两张表（施工 PN-4 起照包编号，设计 48），每张只有 `repo`、`rev`；仓库在 `miyu-agent` 组织下；`rev` 是完整的提交号或者 `v` 开头的
//! 标签。

use std::path::Path;

use toml_edit::{DocumentMut, Item};

/// 相对仓库根。
pub const PATH: &str = "heads.toml";

/// 有哪几个界面，照这个先后。
const HEADS: [&str; 2] = ["miyu-tui", "miyu-webui"];

/// 仓库要在这下面。
const ORG: &str = "https://github.com/miyu-agent/";

/// 查 `root` 下的 `heads.toml`，交回问题，没有问题的是空的。
pub fn check(root: &Path) -> Vec<String> {
    match std::fs::read_to_string(root.join(PATH)) {
        Ok(text) => problems(&text),
        Err(error) => vec![format!("{PATH}：读不了（{error}）")],
    }
}

/// 钉的一个头：编号、仓库、取哪一份（提交号或者标签）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// 包编号：`miyu-tui`、`miyu-webui`。
    pub id: String,
    /// 仓库的网址。
    pub repo: String,
    /// 完整的提交号，或者 `v` 开头的标签。
    pub rev: String,
}

/// 读出两个头钉的那一份，照 [`HEADS`] 的先后（`cargo xtask dist` 用，施工 RL-1）。写法有问题的交回 [`check`] 那几条。
pub fn pins(text: &str) -> Result<Vec<Pin>, Vec<String>> {
    let found = problems(text);
    if !found.is_empty() {
        return Err(found);
    }
    let doc: DocumentMut = text
        .parse()
        .map_err(|error| vec![format!("{PATH}：不是 TOML（{error}）")])?;
    Ok(HEADS
        .iter()
        .map(|head| {
            let field = |key: &str| {
                doc[*head][key]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_default()
            };
            Pin {
                id: (*head).to_string(),
                repo: field("repo"),
                rev: field("rev"),
            }
        })
        .collect())
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
