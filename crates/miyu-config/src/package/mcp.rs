//! 包带的 MCP 服务（施工 X-2，`docs/blueprint/mcp.md`「对外的样子」）：`[[mcp]]` 一段一个，读成 [`McpServer`]。一个服务要么是本机
//! 拉起的一条命令，要么是一个网址（施工 X-32 上，Streamable HTTP），见 [`Reach`]。只查写法；
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
    /// 怎么到它：拉起一条命令，或者连一个网址。
    pub reach: Reach,
    /// 给哪种会话用：`local`、`private`、`group`，照写的先后；没写的只给本机的会话（2026-10-11 项目主人照推荐定）。
    pub venues: Vec<String>,
    /// 一次调用等多久，毫秒。
    pub timeout_ms: u64,
    /// 这个服务的工具都要先加载（施工 L-1，`25-工具加载.md` W9）：没写的先加载（施工 L-3，2026-10-20 项目主人改：一个服务常有
    /// 十几件工具），`false`、`"no"` 全量；`"yes"`、`"no"` 大小写不论。个人设置的 `loading.mcp.<服务名>` 盖过它。
    pub need_load: bool,
    /// 这个服务的工具脚本里能调（施工 M-2，`39-脚本调工具.md` 第三节）：没写的能，`false`、`"no"` 不能，写法同 `need_load`。
    /// 个人设置的 `clicall.mcp.<服务名>`、`clicall.tools.<工具名>` 盖过它。
    pub clicall: bool,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

/// 怎么到一个服务（施工 X-32 上）：`command`、`url` 写一个。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// 本机拉起的进程，经标准输入输出说 MCP。
    Command {
        /// 命令：第一项是程序，后面是参数，不经 shell。
        command: Vec<String>,
        /// 另给它的环境变量，照写的先后：值是字，或者引用一个密钥、一个环境变量。
        env: Vec<(String, EnvValue)>,
    },
    /// 一个网址，经 Streamable HTTP 说 MCP。
    Url {
        /// 网址：`https://`，或者本机地址的 `http://`（本机不本机由核心连的时候认，`mcp.md`「网址的服务怎么说话」）。
        url: String,
        /// 令牌：发成 `Authorization: Bearer <值>`。
        bearer: Option<EnvValue>,
        /// 别的请求头，照写的先后。
        headers: Vec<(String, EnvValue)>,
    },
}

/// 环境变量、令牌、请求头的值。
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
    reader.only(table, "mcp", &super::fields::MCP)?;
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

/// 一个服务除了名字的几格（清单的一段、个人设置的一张表都是这几格）：清单格式的真相源在 `fields`（施工 Y-5 末上）。
use super::fields::MCP_SERVER as FIELDS;

/// 叫 `name` 的服务的几格，表是 `table`，写在第 `line` 行。
fn fields(
    table: &dyn toml_edit::TableLike,
    name: String,
    line: Option<usize>,
) -> Result<McpServer, Problem> {
    let at = format!("mcp.{name}");
    let reach = reach(table, &at, line)?;
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
    let need_load = switch(table, "need_load", &at, line)?;
    let clicall = switch(table, "clicall", &at, line)?;
    Ok(McpServer {
        name,
        reach,
        venues,
        timeout_ms,
        need_load,
        clicall,
        line,
    })
}

/// 怎么到它：`command`（可以带 `env`）、`url`（可以带 `bearer`、`headers`）写一个，混着写、都没写的不收。
fn reach(
    table: &dyn toml_edit::TableLike,
    at: &str,
    line: Option<usize>,
) -> Result<Reach, Problem> {
    let wrong =
        |key: &str, what: &str| bad(line, &format!("{at}.{key}"), format!("{at}.{key} {what}"));
    let values = "must be a table, each value text or { secret = … } or { env = … }";
    if table.get("url").is_none() {
        for key in ["bearer", "headers"] {
            if table.get(key).is_some() {
                return Err(wrong(key, "goes with url, not command"));
            }
        }
        let command = table
            .get("command")
            .and_then(Item::as_array)
            .and_then(texts)
            .filter(|command| !command.is_empty())
            .ok_or_else(|| {
                wrong(
                    "command",
                    "must be a list of words, the program first, none of them empty; or write url instead",
                )
            })?;
        let env = match table.get("env") {
            Some(item) => named(item, valid_variable).ok_or_else(|| wrong("env", values))?,
            None => Vec::new(),
        };
        return Ok(Reach::Command { command, env });
    }
    for key in ["command", "env"] {
        if table.get(key).is_some() {
            return Err(wrong(key, "goes with command, not url: write one of them"));
        }
    }
    let url = table
        .get("url")
        .and_then(Item::as_str)
        .filter(|url| valid_url(url))
        .ok_or_else(|| {
            wrong(
                "url",
                "must start with https:// (or http:// for this machine), no spaces",
            )
        })?
        .to_string();
    let bearer = match table.get("bearer") {
        Some(item) => Some(
            value(item)
                .ok_or_else(|| wrong("bearer", "must be text or { secret = … } or { env = … }"))?,
        ),
        None => None,
    };
    let headers = match table.get("headers") {
        Some(item) => named(item, valid_header).ok_or_else(|| wrong("headers", values))?,
        None => Vec::new(),
    };
    Ok(Reach::Url {
        url,
        bearer,
        headers,
    })
}

/// 网址的写法：`https://` 或 `http://` 开头，后面还有字，没有空白、控制字符。`http://` 只给本机地址，连的时候认。
fn valid_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty())
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// 环境变量名：不是空的，没有 `=` 和 NUL。
fn valid_variable(name: &str) -> bool {
    !name.is_empty() && !name.contains(['=', '\0'])
}

/// 请求头的名字：字母、数字、`-`，不是空的；`Authorization` 归 `bearer`，`Mcp-Session-Id`、`MCP-Protocol-Version` 归核心。
fn valid_header(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !["authorization", "mcp-session-id", "mcp-protocol-version"]
            .contains(&name.to_ascii_lowercase().as_str())
}

/// 一格开关 `key`：`true`、`false`，或者 `"yes"`、`"no"`（大小写不论）；没写的开。
fn switch(
    table: &dyn toml_edit::TableLike,
    key: &str,
    at: &str,
    line: Option<usize>,
) -> Result<bool, Problem> {
    let Some(item) = table.get(key) else {
        return Ok(true);
    };
    item.as_bool()
        .or_else(
            || match item.as_str().map(str::to_ascii_lowercase).as_deref() {
                Some("yes") => Some(true),
                Some("no") => Some(false),
                _ => None,
            },
        )
        .ok_or_else(|| {
            bad(
                line,
                &format!("{at}.{key}"),
                format!("{at}.{key} must be true, false, \"yes\" or \"no\""),
            )
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

/// `env`、`headers`：名字合 `valid` 的，值照 [`value`]；有一项不合的没有。
fn named(item: &Item, valid: fn(&str) -> bool) -> Option<Vec<(String, EnvValue)>> {
    item.as_table_like()?
        .iter()
        .map(|(key, item)| {
            if !valid(key) {
                return None;
            }
            Some((key.to_string(), value(item)?))
        })
        .collect()
}

/// 一个值：字（没有 NUL），或者一个引用（`{ secret = … }`、`{ env = … }`）。
fn value(item: &Item) -> Option<EnvValue> {
    match item {
        Item::Value(Value::String(text)) => {
            let text = text.value();
            (!text.contains('\0')).then(|| EnvValue::Text(text.clone()))
        }
        other => Some(EnvValue::Reference(Reference::read(
            other.as_table_like()?,
        )?)),
    }
}

/// `venues`：照 [`venues_fit`] 查。
fn venues(item: &Item) -> Option<Vec<String>> {
    let list = texts(item.as_array()?)?;
    venues_fit(&list).then_some(list)
}

/// 一串会话种类合不合：至少一个，都是认的种类（`local`、`private`、`group`），不重。清单的 `venues`、脚本开头的 `Venues`
/// （施工 X-43）共用这一份。
pub fn venues_fit(list: &[String]) -> bool {
    !list.is_empty()
        && list.iter().all(|venue| VENUES.contains(&venue.as_str()))
        && list
            .iter()
            .enumerate()
            .all(|(n, venue)| !list[..n].contains(venue))
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
