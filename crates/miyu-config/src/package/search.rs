//! 扩展声明的搜索来源（施工 N-5，设计 36 第二节第 5 条）：`[[search]]` 一段一个，读成 [`SearchSource`]。只查写法；核心起来时
//! 把它们加进设置「搜索服务供应商」的下拉（值是 `<包>/<编号>`），选了它才经 `search.query` 问这个包的程序。只有扩展包能写
//! （`kinds::tables`），要在 `[process] capabilities` 里声明 `search`（[`needs_capability`]，读完整份以后查）。

use toml_edit::Item;

use super::capability::Capability;
use super::reader::{Reader, command_name, line_of};
use super::{Code, Problem};
use crate::phrases::Phrases;

/// 扩展的来源进的那一项设置：联网包的「搜索服务供应商」。
pub const SEARCH_SOURCE: &str = "net.search.source";

/// 扩展声明的一个搜索来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSource {
    /// 编号：写法同命令名（小写字母开头，小写字母、数字、`-`，最多 32 个），一个包里不重。
    pub id: String,
    /// 给人看的名字，照语言。
    pub name: Phrases,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

impl SearchSource {
    /// 下拉里的值：`<包>/<编号>`，和出厂的几家撞不上（它们不带 `/`）。
    pub fn option(&self, package: &str) -> String {
        format!("{package}/{}", self.id)
    }
}

/// `[[search]]`：一段一个，编号在一个包里不重。
pub(super) fn read(reader: &Reader<'_>, node: &Item) -> Result<Vec<SearchSource>, Problem> {
    let Some(array) = node.as_array_of_tables() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "search",
            "search is written as [[search]], one table per source".to_string(),
        ));
    };
    let mut sources: Vec<SearchSource> = Vec::new();
    for table in array.iter() {
        let line = table.span().map(|span| line_of(reader.text, span.start));
        reader.only(table, "search", super::fields::SEARCH)?;
        let id = match table.get("id").and_then(Item::as_str) {
            Some(id) if command_name(id) => id.to_string(),
            Some(id) => {
                return Err(bad(
                    line,
                    &format!("search.id {id:?}"),
                    format!(
                        "search source id {id:?} must start with a lowercase letter, use only lowercase letters, digits and -, at most 32"
                    ),
                ));
            }
            None => {
                return Err(bad(
                    line,
                    "search.id",
                    "search.id is missing or not text".to_string(),
                ));
            }
        };
        let at = format!("search.{id}");
        if sources.iter().any(|seen| seen.id == id) {
            return Err(bad(
                line,
                &at,
                format!("search source {id:?} is written twice"),
            ));
        }
        let name = match table.get("name") {
            Some(item) => reader.phrases(item, &format!("{at}.name"))?,
            None => {
                return Err(bad(
                    line,
                    &format!("{at}.name"),
                    format!("{at}.name is missing"),
                ));
            }
        };
        sources.push(SearchSource { id, name, line });
    }
    Ok(sources)
}

/// 声明了搜索来源、`[process] capabilities` 里却没有 `search` 的：报在第一段。
pub(super) fn needs_capability(
    sources: &[SearchSource],
    capabilities: &[Capability],
) -> Result<(), Problem> {
    match sources.first() {
        Some(first) if !capabilities.contains(&Capability::Search) => Err(bad(
            first.line,
            "search",
            "[[search]] needs the search capability in [process] capabilities".to_string(),
        )),
        _ => Ok(()),
    }
}

/// 写错了的一处。
fn bad(line: Option<usize>, key: &str, message: String) -> Problem {
    Problem {
        line,
        code: Code::BadSearch,
        detail: key.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
