//! 工具调用后的通知（施工 HK-2，设计 `65-工具挂接点.md` 第四节第 2 条）：`[after]` 一张表，读成 [`After`]。写法同 `[guard]`：
//! 收哪几件工具（`tools`）、哪几类访问（`access`）的，两样至少写一样（第十节第 1 条：不许「什么都收」）。通知不等回答，没有
//! 时限这一格。只有扩展包能写（`kinds::tables`），要在 `[process] capabilities` 里声明 `events.read`（第十节第 1 条：
//! `tool.after` 用 `events.read`，不另加能力；读完整份以后查，同 `[guard]`）。核心起来时照它建表，结果落盘以后查一次表。

use toml_edit::Item;

use super::capability::Capability;
use super::guard::{bad_as, covering};
use super::reader::{Reader, line_of};
use super::{Code, Problem};

/// 一个扩展要收的通知：哪些调用的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct After {
    /// 工具名：调用的工具名在里面的发。照写的先后，去掉重复；没写的是空的。
    pub tools: Vec<String>,
    /// 访问类别（同 `[guard]` 的那几个）：调用的访问类别在里面的发。照写的先后，去掉重复；没写的是空的。
    pub access: Vec<String>,
    /// 这张表写在第几行。
    pub line: Option<usize>,
}

impl After {
    /// 工具 `tool`、访问类别 `access`（不认识的工具没有）的这次调用发不发给它。
    pub fn covers(&self, tool: &str, access: Option<&str>) -> bool {
        self.tools.iter().any(|one| one == tool)
            || access.is_some_and(|access| self.access.iter().any(|one| one == access))
    }
}

/// `[after]`：没写的没有。
pub(super) fn read(
    reader: &Reader<'_>,
    node: Option<&Item>,
    capabilities: &[Capability],
) -> Result<Option<After>, Problem> {
    let Some(node) = node else {
        return Ok(None);
    };
    let line = node.span().map(|span| line_of(reader.text, span.start));
    let Some(table) = node.as_table_like() else {
        return Err(bad_as(
            Code::BadAfter,
            line,
            "after",
            "after must be a table".to_string(),
        ));
    };
    reader.only(table, "after", super::fields::AFTER)?;
    let (tools, access) = covering(reader, table, ("after", Code::BadAfter), line)?;
    if !capabilities.contains(&Capability::EventsRead) {
        return Err(bad_as(
            Code::BadAfter,
            line,
            "after",
            "[after] needs the events.read capability in [process] capabilities".to_string(),
        ));
    }
    Ok(Some(After {
        tools,
        access,
        line,
    }))
}

#[cfg(test)]
mod tests;
