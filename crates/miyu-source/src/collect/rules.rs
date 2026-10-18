//! 收录的规则（施工 G-2 下，设计 `37-软件源.md` 第三节第 4 条）：`miyu-agent/packages` 仓库里的四份 TOML。
//!
//! - `sources.toml`：照哪几种来源收，`[[source]] kind = "official"`；以后社区的是 `kind = "topic"`、`topic = "…"`（第三节第
//!   5 条），现在读到了照样认、收的时候跳过并记一笔。
//! - `official.toml`：`repos = ["miyu-agent/miyu-embed", …]`，项目主人自己的仓库。
//! - `names.toml`：`[names]` 下编号到来源仓库，先到先得、永远跟着它；收录时新占的写回去。
//! - `blocked.toml`：`[[blocked]]` 每条写 `id` 或 `origin`（或两个都写）和 `reason`。

use std::collections::BTreeMap;
use std::path::Path;

use toml_edit::{Document, Item, TableLike};

use crate::index::Blocked;

/// 一种来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// `official.toml` 列的仓库。
    Official,
    /// 打了这个话题标签的仓库（社区自动收录，以后做）。
    Topic(String),
}

/// 四份规则读好的样子。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rules {
    /// 照哪几种来源收，照写的先后。
    pub sources: Vec<Source>,
    /// 官方的仓库：`owner/repo`。
    pub official: Vec<String>,
    /// 编号到来源仓库（`github.com/owner/repo`）。
    pub names: BTreeMap<String, String>,
    /// 拉黑的。
    pub blocked: Vec<Blocked>,
}

impl Rules {
    /// 读仓库目录 `dir` 里的四份；没有的文件当空的。
    ///
    /// # Errors
    ///
    /// 读不了、写错了：哪个文件、哪里不对，英文一句。
    pub fn read(dir: &Path) -> Result<Rules, String> {
        let file = |name: &str| -> Result<Option<(String, Document<String>)>, String> {
            let path = dir.join(name);
            if !path.exists() {
                return Ok(None);
            }
            let text =
                std::fs::read_to_string(&path).map_err(|error| format!("{name}: {error}"))?;
            Document::parse(text.clone())
                .map(|document| Some((text, document)))
                .map_err(|error| format!("{name}: {}", error.message().trim()))
        };
        let mut rules = Rules::default();
        if let Some((_, document)) = file("sources.toml")? {
            rules.sources = sources(document.as_table())?;
        }
        if let Some((_, document)) = file("official.toml")? {
            rules.official = texts(document.as_table(), "official.toml", "repos")?;
            if let Some(bad) = rules.official.iter().find(|repo| !valid_repo(repo)) {
                return Err(format!("official.toml: \"{bad}\" is not owner/repo"));
            }
        }
        if let Some((_, document)) = file("names.toml")? {
            rules.names = names(document.as_table())?;
        }
        if let Some((_, document)) = file("blocked.toml")? {
            rules.blocked = blocked(document.as_table())?;
        }
        Ok(rules)
    }
}

/// `owner/repo`：两段，GitHub 认的字（字母、数字、`-`、`_`、`.`），都不是空的、不是 `.`、`..`。
pub fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    [owner, name].iter().all(|part| {
        !part.is_empty()
            && *part != "."
            && *part != ".."
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    })
}

/// 仓库 `owner/repo` 的来源写法。
pub fn origin(repo: &str) -> String {
    format!("github.com/{repo}")
}

fn sources(root: &dyn TableLike) -> Result<Vec<Source>, String> {
    let Some(item) = root.get("source") else {
        return Ok(Vec::new());
    };
    let tables = item
        .as_array_of_tables()
        .ok_or("sources.toml: source must be [[source]] tables")?;
    tables
        .iter()
        .map(|table| match table.get("kind").and_then(Item::as_str) {
            Some("official") => Ok(Source::Official),
            Some("topic") => table
                .get("topic")
                .and_then(Item::as_str)
                .filter(|topic| !topic.is_empty())
                .map(|topic| Source::Topic(topic.to_string()))
                .ok_or_else(|| "sources.toml: a topic source needs topic".to_string()),
            other => Err(format!("sources.toml: unknown kind {other:?}")),
        })
        .collect()
}

fn texts(root: &dyn TableLike, file: &str, key: &str) -> Result<Vec<String>, String> {
    let Some(item) = root.get(key) else {
        return Ok(Vec::new());
    };
    item.as_array()
        .and_then(|array| {
            array
                .iter()
                .map(|value| value.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| format!("{file}: {key} must be a list of text"))
}

fn names(root: &dyn TableLike) -> Result<BTreeMap<String, String>, String> {
    let Some(item) = root.get("names") else {
        return Ok(BTreeMap::new());
    };
    let table = item
        .as_table_like()
        .ok_or("names.toml: names must be a table")?;
    table
        .iter()
        .map(|(id, value)| {
            value
                .as_str()
                .map(|origin| (id.to_string(), origin.to_string()))
                .ok_or_else(|| format!("names.toml: names.{id} must be text"))
        })
        .collect()
}

fn blocked(root: &dyn TableLike) -> Result<Vec<Blocked>, String> {
    let Some(item) = root.get("blocked") else {
        return Ok(Vec::new());
    };
    let tables = item
        .as_array_of_tables()
        .ok_or("blocked.toml: blocked must be [[blocked]] tables")?;
    tables
        .iter()
        .map(|table| {
            let text = |key: &str| table.get(key).and_then(Item::as_str).map(str::to_string);
            let entry = Blocked {
                id: text("id"),
                origin: text("origin"),
                reason: text("reason").unwrap_or_default(),
            };
            if (entry.id.is_none() && entry.origin.is_none()) || entry.reason.trim().is_empty() {
                return Err("blocked.toml: each entry needs id or origin, and a reason".to_string());
            }
            Ok(entry)
        })
        .collect()
}

/// `names.toml` 的全文：照编号排，一行一个。
pub fn names_text(names: &BTreeMap<String, String>) -> String {
    let mut text = String::from(
        "# Package ids belong to the first repository that published them, for good (design 37, section 3).\n[names]\n",
    );
    for (id, origin) in names {
        text.push_str(&format!("{id} = \"{origin}\"\n"));
    }
    text
}
