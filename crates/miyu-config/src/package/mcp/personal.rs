//! 个人设置里的 MCP 服务（施工 F-9 上，`docs/blueprint/mcp.md`「个人设置里的一张表」）：一个服务一张 `[mcp.<名字>]`，名字
//! 照清单的写法，格子同清单的 `[[mcp]]`、不写 `name`。不进配置清单：读个人设置的那一边跳过这一张表（`crate::parse`）。
//! 写错了的那一个不收、记一条问题，别的照收（G8）。

use toml_edit::{Document, Item};

use super::super::reader::{Reader, line_of};
use super::super::{Code, Problem};
use super::{FIELDS, McpServer, NAME_CHARS, bad, fields, valid_name};

/// 个人设置的字 `text` 里的服务，照写的先后，和写错了的几处。整份不是 TOML 的一个都没有、也不另报：配置那一边报 `syntax`。
pub fn personal(text: &str) -> (Vec<McpServer>, Vec<Problem>) {
    let Ok(document) = Document::parse(text) else {
        return (Vec::new(), Vec::new());
    };
    let reader = Reader { text };
    let Some(node) = document.as_table().get("mcp") else {
        return (Vec::new(), Vec::new());
    };
    let Some(table) = node.as_table_like() else {
        let problem = reader.problem(
            Some(node),
            Code::BadMcp,
            "mcp",
            "mcp is written as [mcp.<name>], one table per server".to_string(),
        );
        return (Vec::new(), vec![problem]);
    };
    let mut servers = Vec::new();
    let mut problems = Vec::new();
    for (name, item) in table.iter() {
        let line = table
            .get_key_value(name)
            .and_then(|(key, _)| key.span())
            .map(|span| line_of(text, span.start));
        match one(&reader, name, item, line) {
            Ok(server) => servers.push(server),
            Err(problem) => problems.push(problem),
        }
    }
    (servers, problems)
}

/// 叫 `name` 的一张表 `item`，名字写在第 `line` 行。
fn one(
    reader: &Reader<'_>,
    name: &str,
    item: &Item,
    line: Option<usize>,
) -> Result<McpServer, Problem> {
    let at = format!("mcp.{name}");
    if !valid_name(name) {
        return Err(bad(
            line,
            &format!("mcp {name:?}"),
            format!(
                "MCP server name {name:?} must start with a lowercase letter, use only lowercase letters, digits and -, at most {NAME_CHARS}"
            ),
        ));
    }
    let Some(table) = item.as_table_like() else {
        return Err(bad(
            line,
            &at,
            format!("{at} is written as a table: [{at}]"),
        ));
    };
    reader.only(table, &at, &FIELDS)?;
    fields(table, name.to_string(), line)
}

#[cfg(test)]
mod tests;
