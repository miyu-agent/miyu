//! 扩展的守卫（施工 HK-1，设计 `65-工具挂接点.md` 第三节第 2 条）：`[guard]` 一张表，读成 [`Guard`]。只查写法：问哪几件工具
//! （`tools`）、哪几类访问（`access`），两样至少写一样（第十节第 1 条：不许「什么都问」）；时限 `timeout_ms` 不写是
//! 1000 毫秒，只能在 100 到 5000 之间。只有扩展包能写（`kinds::tables`），要在
//! `[process] capabilities` 里声明 `tool.guard`（读完整份以后查，同 `[[search]]`）。核心起来时照它建表，调用前查一次表。

use toml_edit::{Item, TableLike};

use super::capability::Capability;
use super::reader::{Reader, line_of};
use super::{Code, Problem};

/// 不写时等多久：毫秒（设计 65 第八节第 5 条）。
const TIMEOUT_DEFAULT: u64 = 1000;

/// 能写的最短时限：毫秒。
const TIMEOUT_MIN: u64 = 100;

/// 能写的最长时限：毫秒，免得一次调用被卡太久。
const TIMEOUT_MAX: u64 = 5000;

/// `access` 认的访问类别，同内核的访问类别（`miyu_kernel::tool::Access`）：这个 crate 不依赖内核，照写一份，协议端点的测试
/// 对着内核那一份查。
pub const ACCESS: [&str; 7] = [
    "read", "write", "execute", "network", "outbound", "venue", "install",
];

/// 一个扩展的守卫：问哪些调用、等多久。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guard {
    /// 工具名：调用的工具名在里面的问。照写的先后，去掉重复；没写的是空的。
    pub tools: Vec<String>,
    /// 访问类别（[`ACCESS`] 里的）：调用的访问类别在里面的问。照写的先后，去掉重复；没写的是空的。
    pub access: Vec<String>,
    /// 等它答多久，毫秒。
    pub timeout_ms: u64,
    /// 这张表写在第几行。
    pub line: Option<usize>,
}

impl Guard {
    /// 工具 `tool`、访问类别 `access` 的这次调用问不问它。
    pub fn covers(&self, tool: &str, access: &str) -> bool {
        self.tools.iter().any(|one| one == tool) || self.access.iter().any(|one| one == access)
    }
}

/// `[guard]`：没写的没有。
pub(super) fn read(
    reader: &Reader<'_>,
    node: Option<&Item>,
    capabilities: &[Capability],
) -> Result<Option<Guard>, Problem> {
    let Some(node) = node else {
        return Ok(None);
    };
    let line = node.span().map(|span| line_of(reader.text, span.start));
    let Some(table) = node.as_table_like() else {
        return Err(bad(line, "guard", "guard must be a table".to_string()));
    };
    reader.only(table, "guard", super::fields::GUARD)?;
    let (tools, access) = covering(reader, table, ("guard", Code::BadGuard), line)?;
    let timeout_ms = match table.get("timeout_ms") {
        Some(item) => item
            .as_integer()
            .and_then(|ms| u64::try_from(ms).ok())
            .filter(|ms| (TIMEOUT_MIN..=TIMEOUT_MAX).contains(ms))
            .ok_or_else(|| {
                bad(
                    line,
                    "guard.timeout_ms",
                    format!(
                        "guard.timeout_ms must be a whole number from {TIMEOUT_MIN} to {TIMEOUT_MAX}"
                    ),
                )
            })?,
        None => TIMEOUT_DEFAULT,
    };
    if !capabilities.contains(&Capability::ToolGuard) {
        return Err(bad(
            line,
            "guard",
            "[guard] needs the tool.guard capability in [process] capabilities".to_string(),
        ));
    }
    Ok(Some(Guard {
        tools,
        access,
        timeout_ms,
        line,
    }))
}

/// 表 `name`（`guard`、`after`，写错的报 `code`）的 `tools`、`access`：工具名不像样、访问类别不认识、两样都没写的读不进
/// （第十节第 1 条：不许「什么都问」；`[after]` 同写法，第四节第 2 条，施工 HK-2）。
pub(super) fn covering(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    (name, code): (&str, Code),
    line: Option<usize>,
) -> Result<(Vec<String>, Vec<String>), Problem> {
    let tools = names(reader, table, (name, code), "tools", line)?;
    if let Some(tool) = tools.iter().find(|tool| !tool_name(tool)) {
        return Err(bad_as(
            code,
            line,
            &format!("{name}.tools {tool:?}"),
            format!("{name}.tools {tool:?} is not a tool name: 1 to 128 characters, no spaces"),
        ));
    }
    let access = names(reader, table, (name, code), "access", line)?;
    if let Some(one) = access.iter().find(|one| !ACCESS.contains(&one.as_str())) {
        return Err(bad_as(
            code,
            line,
            &format!("{name}.access {one:?}"),
            format!("{name}.access {one:?} is not one of {}", ACCESS.join(", ")),
        ));
    }
    if tools.is_empty() && access.is_empty() {
        return Err(bad_as(
            code,
            line,
            name,
            format!("[{name}] needs tools or access: it can't take every call"),
        ));
    }
    Ok((tools, access))
}

/// 一格字的数组，去掉重复、照写的先后。
fn names(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    (name, code): (&str, Code),
    key: &str,
    line: Option<usize>,
) -> Result<Vec<String>, Problem> {
    let mut names: Vec<String> = Vec::new();
    for name in reader
        .texts(table, name, key)
        .map_err(|problem| bad_as(code, line, &problem.detail, problem.message))?
    {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Ok(names)
}

/// 工具名的写法：1 到 128 个字，没有空白、控制字符。工具名的细规矩由工具目录管，这里只挡明显写错的。
fn tool_name(name: &str) -> bool {
    (1..=128).contains(&name.chars().count())
        && !name.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// 写错了的一处。
fn bad(line: Option<usize>, key: &str, message: String) -> Problem {
    bad_as(Code::BadGuard, line, key, message)
}

/// 写错了的一处，报 `code`。
pub(super) fn bad_as(code: Code, line: Option<usize>, key: &str, message: String) -> Problem {
    Problem {
        line,
        code,
        detail: key.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
