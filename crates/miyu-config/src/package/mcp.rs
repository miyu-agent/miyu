//! 包带的 MCP 服务（施工 X-2，`docs/blueprint/mcp.md`「对外的样子」）：`[[mcp]]` 一段一个，读成 [`McpServer`]。只查写法；
//! 怎么起、怎么登记工具在核心那一边（X-3）。一个服务一个功能（[`McpServer::feature_id`]），编号全局不重，两个包撞了照功能
//! 编号撞了报（`miyu-store` 的 `feature_taken`）。个人设置里的 `[mcp.<名字>]` 照同样的格子读（施工 F-9 上，`personal.rs`）。

mod personal;

pub use personal::personal;

use toml_edit::{Item, Value};

use super::reader::{Reader, line_of};
use super::{Code, Problem};
use crate::secret::Reference;

/// 服务名最多几个字符：工具名 `mcp__<服务名>__<工具名>` 最长 64 个，留给工具名的不能太少。
const NAME_CHARS: usize = 32;

/// 一次调用最少等多久，毫秒。
pub const TIMEOUT_MIN: u64 = 1_000;

/// 一次调用最多等多久，毫秒：一小时。
pub const TIMEOUT_MAX: u64 = 3_600_000;

/// 没写的等多久，毫秒：十分钟（浏览器这类服务一次调用就要好一阵）。
pub const TIMEOUT_DEFAULT: u64 = 600_000;

/// 认的会话种类，同扩展登记工具的 `venues`（`providers.md`）。
const VENUES: [&str; 3] = ["local", "private", "group"];

/// 一个 MCP 服务。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    /// 服务名：小写字母开头，只有小写字母、数字、`-`，最多 32 个。
    pub name: String,
    /// 拉起它的命令：第一项是程序，后面是参数，不经 shell。
    pub command: Vec<String>,
    /// 另给它的环境变量，照写的先后：值是字，或者引用一个密钥、一个环境变量。
    pub env: Vec<(String, EnvValue)>,
    /// 给哪种会话用：`local`、`private`、`group`，照写的先后；没写的只给本机的会话（2026-10-11 项目主人照推荐定）。
    pub venues: Vec<String>,
    /// 一次调用等多久，毫秒。
    pub timeout_ms: u64,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

/// 环境变量的值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvValue {
    /// 照原样。
    Text(String),
    /// `{ secret = … }`、`{ env = … }`：起它的那一刻照核心的密钥文件、环境取。
    Reference(Reference),
}

impl McpServer {
    /// 它的功能的编号：`mcp-<服务名>`。
    pub fn feature_id(&self) -> String {
        format!("mcp-{}", self.name)
    }
}

/// `[[mcp]]`：一段一个，服务名在一个包里不重。
pub(super) fn read(reader: &Reader<'_>, node: &Item) -> Result<Vec<McpServer>, Problem> {
    let Some(array) = node.as_array_of_tables() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "mcp",
            "mcp is written as [[mcp]], one table per server".to_string(),
        ));
    };
    let mut servers: Vec<McpServer> = Vec::new();
    for table in array.iter() {
        let line = table.span().map(|span| line_of(reader.text, span.start));
        let server = one(reader, table, line)?;
        if servers.iter().any(|seen| seen.name == server.name) {
            return Err(bad(
                line,
                &format!("mcp.{}", server.name),
                format!("MCP server {:?} is written twice", server.name),
            ));
        }
        servers.push(server);
    }
    Ok(servers)
}

/// 一段。
fn one(
    reader: &Reader<'_>,
    table: &toml_edit::Table,
    line: Option<usize>,
) -> Result<McpServer, Problem> {
    reader.only(
        table,
        "mcp",
        &["name", FIELDS[0], FIELDS[1], FIELDS[2], FIELDS[3]],
    )?;
    let name = match table.get("name").and_then(Item::as_str) {
        Some(name) if valid_name(name) => name.to_string(),
        Some(name) => {
            return Err(bad(
                line,
                &format!("mcp.name {name:?}"),
                format!(
                    "MCP server name {name:?} must start with a lowercase letter, use only lowercase letters, digits and -, at most {NAME_CHARS}"
                ),
            ));
        }
        None => {
            return Err(bad(
                line,
                "mcp.name",
                "mcp.name is missing or not text".to_string(),
            ));
        }
    };
    fields(table, name, line)
}

/// 一个服务除了名字的几格（清单的一段、个人设置的一张表都是这几格）。
const FIELDS: [&str; 4] = ["command", "env", "venues", "timeout_ms"];

/// 叫 `name` 的服务的几格，表是 `table`，写在第 `line` 行。
fn fields(
    table: &dyn toml_edit::TableLike,
    name: String,
    line: Option<usize>,
) -> Result<McpServer, Problem> {
    let at = format!("mcp.{name}");
    let command = match table.get("command").and_then(Item::as_array) {
        Some(array) => texts(array).filter(|command| !command.is_empty()),
        None => None,
    }
    .ok_or_else(|| {
        bad(
            line,
            &format!("{at}.command"),
            format!("{at}.command must be a list of words, the program first, none of them empty"),
        )
    })?;
    let env = match table.get("env") {
        Some(item) => env(item).ok_or_else(|| {
            bad(
                line,
                &format!("{at}.env"),
                format!(
                    "{at}.env must be a table of variables, each text or {{ secret = … }} or {{ env = … }}"
                ),
            )
        })?,
        None => Vec::new(),
    };
    let venues = match table.get("venues") {
        Some(item) => venues(item).ok_or_else(|| {
            bad(
                line,
                &format!("{at}.venues"),
                format!("{at}.venues must list some of local, private, group, each once"),
            )
        })?,
        None => vec!["local".to_string()],
    };
    let timeout_ms = match table.get("timeout_ms") {
        Some(item) => item
            .as_integer()
            .and_then(|ms| u64::try_from(ms).ok())
            .filter(|ms| (TIMEOUT_MIN..=TIMEOUT_MAX).contains(ms))
            .ok_or_else(|| {
                bad(
                    line,
                    &format!("{at}.timeout_ms"),
                    format!(
                        "{at}.timeout_ms must be a whole number from {TIMEOUT_MIN} to {TIMEOUT_MAX}"
                    ),
                )
            })?,
        None => TIMEOUT_DEFAULT,
    };
    Ok(McpServer {
        name,
        command,
        env,
        venues,
        timeout_ms,
        line,
    })
}

/// 服务名合不合写法。
fn valid_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().count() <= NAME_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// 一串字，每一个都不是空的、没有 NUL；有不是字的没有。
fn texts(array: &toml_edit::Array) -> Option<Vec<String>> {
    array
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|text| !text.is_empty() && !text.contains('\0'))
                .map(str::to_string)
        })
        .collect()
}

/// `env`：变量名不是空的、没有 `=` 和 NUL；值是字（没有 NUL），或者一个引用。
fn env(item: &Item) -> Option<Vec<(String, EnvValue)>> {
    let table = item.as_table_like()?;
    table
        .iter()
        .map(|(key, value)| {
            if key.is_empty() || key.contains(['=', '\0']) {
                return None;
            }
            let value = match value {
                Item::Value(Value::String(text)) => {
                    let text = text.value();
                    (!text.contains('\0')).then(|| EnvValue::Text(text.clone()))?
                }
                other => EnvValue::Reference(Reference::read(other.as_table_like()?)?),
            };
            Some((key.to_string(), value))
        })
        .collect()
}

/// `venues`：至少一个，都是认的种类，不重。
fn venues(item: &Item) -> Option<Vec<String>> {
    let list = texts(item.as_array()?)?;
    let fits = !list.is_empty()
        && list.iter().all(|venue| VENUES.contains(&venue.as_str()))
        && list
            .iter()
            .enumerate()
            .all(|(n, venue)| !list[..n].contains(venue));
    fits.then_some(list)
}

/// 写错了的一处。
fn bad(line: Option<usize>, detail: &str, message: String) -> Problem {
    Problem {
        line,
        code: Code::BadMcp,
        detail: detail.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
