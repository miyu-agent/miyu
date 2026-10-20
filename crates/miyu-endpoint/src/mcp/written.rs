//! 个人设置里一张 `[mcp.<名字>]` 的字（施工 X-26，`docs/blueprint/mcp.md`「个人设置里的一张表」第 2 条）：`mcp.set` 加上、
//! 整条换掉，`mcp.remove` 删掉。纯字的活，不碰磁盘。
//!
//! 同 `miyu_config::edit` 的做法：在原来的字上按位置换，不把整份读成可改的文档再写回去，别的字节一个不变（`07-存储.md`
//! S9）。一个服务的字是它的表头那张表（连同点号连着写在里面的、行尾注释）、它下面的子表（`[mcp.<名字>.env]`），或者
//! `[mcp]` 里点号连着写的几行、一行行内表。
//!
//! - 加上：接在文件末尾，前面空一行。
//! - 换掉：原来写成有表头的表的，新的一张放在原来表头的位置（表头前面的注释留着）；写成别的样子的删掉那几行，新的接在末尾。
//! - 删掉：连同表后面的空行；在文件末尾的，连前面的空行。
//! - 换行照这份文件原来的。`mcp` 本身写成了值、行内表、`[[mcp]]` 的放不进去（[`None`]）；改完再读一遍，读不懂、读出来的
//!   不是要写的那一个的，也算放不进去。

use std::ops::Range;

use serde_json::Value as Json;
use toml_edit::{Document, Item as Node, Table};

use miyu_config::Value;
use miyu_config::key::join;
use miyu_config::package::{McpServer, personal_mcp};

/// 要写的一个服务：几格照协议交来的，还没查过写法（查法照读服务表的，[`personal_mcp`]）。
#[derive(Clone)]
pub(super) struct Entry {
    /// 服务名：表名。
    pub(super) name: String,
    /// 命令：程序在前，参数在后；没交的（网址的服务）是空的，不写。
    pub(super) command: Vec<String>,
    /// 环境变量，照交来的先后。
    pub(super) env: Vec<(String, Json)>,
    /// 网址（施工 X-32 下）；没交的不写。
    pub(super) url: Option<String>,
    /// 令牌；没交的不写。
    pub(super) bearer: Option<Json>,
    /// 别的请求头，照交来的先后。
    pub(super) headers: Vec<(String, Json)>,
    /// 给哪种会话；没交的不写。
    pub(super) venues: Option<Vec<String>>,
    /// 一次调用等多久；没交的不写。
    pub(super) timeout_ms: Option<i64>,
    /// 要先加载（`25-工具加载.md` W9）；没交的不写，照读服务表时的默认。
    pub(super) need_load: Option<bool>,
    /// 脚本里能调（施工 M-2）；没交的不写，照读服务表时的默认。
    pub(super) clicall: Option<bool>,
}

/// 这个服务的一张表的字，换行是 `\n`；环境变量的值写不成 TOML 的（JSON 的 `null`）是空的。
pub(super) fn block(entry: &Entry) -> Option<String> {
    let list = |words: &[String]| {
        Value::List(
            words
                .iter()
                .map(|word| Value::Text(word.clone().into()))
                .collect(),
        )
        .toml()
    };
    let mut lines = vec![format!("[{}]", join(&["mcp", entry.name.as_str()]))];
    if !entry.command.is_empty() {
        lines.push(format!("command = {}", list(&entry.command)));
    }
    if let Some(url) = &entry.url {
        lines.push(format!("url = {}", Value::Text(url.clone().into()).toml()));
    }
    if !entry.env.is_empty() {
        lines.push(format!("env = {}", table(&entry.env)?));
    }
    if let Some(bearer) = &entry.bearer {
        lines.push(format!("bearer = {}", literal(bearer)?));
    }
    if !entry.headers.is_empty() {
        lines.push(format!("headers = {}", table(&entry.headers)?));
    }
    if let Some(venues) = &entry.venues {
        lines.push(format!("venues = {}", list(venues)));
    }
    if let Some(ms) = entry.timeout_ms {
        lines.push(format!("timeout_ms = {ms}"));
    }
    if let Some(need_load) = entry.need_load {
        lines.push(format!("need_load = {need_load}"));
    }
    if let Some(clicall) = entry.clicall {
        lines.push(format!("clicall = {clicall}"));
    }
    Some(lines.iter().map(|line| format!("{line}\n")).collect())
}

/// 几对名字和值写成一行行内表：`{ A = "b", C = { secret = "d" } }`。值写不成的是空的。
fn table(pairs: &[(String, Json)]) -> Option<String> {
    let pairs = pairs
        .iter()
        .map(|(name, value)| Some(format!("{} = {}", join(&[name]), literal(value)?)))
        .collect::<Option<Vec<_>>>()?;
    Some(format!("{{ {} }}", pairs.join(", ")))
}

/// JSON 的值写成 TOML：字、数、开关照样；数组、对象写成数组、行内表。`null` 写不成。写出来对不对由读的一方查。
fn literal(value: &Json) -> Option<String> {
    Some(match value {
        Json::Null => return None,
        Json::Bool(on) => on.to_string(),
        Json::Number(number) => number.to_string(),
        Json::String(text) => Value::Text(text.clone().into()).toml(),
        Json::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(literal)
                .collect::<Option<Vec<_>>>()?
                .join(", ")
        ),
        Json::Object(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(key, value)| Some(format!("{} = {}", join(&[key]), literal(value)?)))
                .collect::<Option<Vec<_>>>()?
                .join(", ")
        ),
    })
}

/// 字 `text` 里服务 `name` 写了脚本里不能调（`clicall = false`，施工 M-2）：网页的编辑页还不认这一格，`mcp.set` 没交它的
/// 时候照原来写的留着，编辑一次别的格不会把它丢了。
pub(super) fn kept_clicall(text: &str, name: &str) -> Option<bool> {
    let (servers, _) = personal_mcp(text);
    servers
        .into_iter()
        .find(|server| server.name == name && !server.clicall)
        .map(|_| false)
}

/// 字 `text` 里写没写服务 `name`（写错了的也算写了）；`mcp` 写成了放不下的样子的是空的。
pub(super) fn has(text: &str, name: &str) -> Option<bool> {
    Some(!pieces(text, name)?.is_empty())
}

/// 在字 `text` 里加上或者整条换上服务 `name`，新的一张是 `block`（[`block`] 写的）。放不进去的是空的。
pub(super) fn put(text: &str, name: &str, block: &str) -> Option<String> {
    let wanted = only(block)?;
    let mut found = pieces(text, name)?;
    let nl = newline(text);
    let block = block.replace('\n', nl);
    let at = found
        .iter()
        .filter(|piece| piece.header)
        .map(|piece| piece.range.start)
        .min();
    // 新的一张放在第一张表的位置，别的子表连同后面的空行删掉。
    for piece in found
        .iter_mut()
        .filter(|piece| piece.header && Some(piece.range.start) != at)
    {
        piece.range.end = blank_after(text, piece.range.end);
    }
    let cut = cut(text, &found);
    let edited = match at {
        Some(at) => format!("{}{block}{}", &cut[..at], &cut[at..]),
        None => appended(&cut, &block, nl),
    };
    let (servers, _) = personal_mcp(&edited);
    let written = servers.into_iter().find(|server| server.name == name)?;
    (Document::parse(edited.as_str()).is_ok() && same(&written, &wanted)).then_some(edited)
}

/// 从字 `text` 里删掉服务 `name`；本来就没写的照原样。放不进去的是空的。
pub(super) fn remove(text: &str, name: &str) -> Option<String> {
    let mut found = pieces(text, name)?;
    for piece in found.iter_mut().filter(|piece| piece.header) {
        let end = blank_after(text, piece.range.end);
        let mut start = piece.range.start;
        while end == text.len()
            && start > 0
            && text[line_start(text, start - 1)..start].trim().is_empty()
        {
            start = line_start(text, start - 1);
        }
        piece.range = start..end;
    }
    let edited = cut(text, &found);
    Document::parse(edited.as_str()).is_ok().then_some(edited)
}

/// 一张表的字里读出来的那一个服务；写错了、不是正好一个的是空的。
fn only(block: &str) -> Option<McpServer> {
    let (mut servers, problems) = personal_mcp(block);
    (problems.is_empty() && servers.len() == 1).then(|| servers.remove(0))
}

/// 两个服务一样：除了写在第几行。
fn same(a: &McpServer, b: &McpServer) -> bool {
    McpServer {
        line: None,
        ..a.clone()
    } == McpServer {
        line: None,
        ..b.clone()
    }
}

/// 服务的字的一段：在哪，是不是一张有表头的表。
struct Piece {
    range: Range<usize>,
    header: bool,
}

/// 服务 `name` 的字在哪几段，照先后；没写的是空的。整份读不懂、`mcp` 写成放不下的样子的没有。
fn pieces(text: &str, name: &str) -> Option<Vec<Piece>> {
    let document = Document::parse(text).ok()?;
    let mut found = Vec::new();
    match document.get("mcp") {
        None => {}
        Some(Node::Table(table)) => {
            if let Some(node) = table.get(name) {
                collect(text, node, &mut found)?;
            }
        }
        Some(_) => return None,
    }
    found.sort_by_key(|piece| piece.range.start);
    Some(found)
}

/// 一格 `node` 的字：有表头的表是表头那一行到它最后一个值那一行，子表各算一段；点号连着写的、值是它们那几行。
fn collect(text: &str, node: &Node, found: &mut Vec<Piece>) -> Option<()> {
    match node {
        Node::Table(table) if table.is_dotted() => {
            for (_, inner) in table.iter() {
                collect(text, inner, found)?;
            }
        }
        Node::Table(table) => {
            if !table.is_implicit() {
                found.push(header(text, table)?);
            }
            for (_, inner) in table.iter() {
                match inner {
                    Node::Table(child) if !child.is_dotted() => collect(text, inner, found)?,
                    Node::ArrayOfTables(array) => {
                        for child in array.iter() {
                            found.push(header(text, child)?);
                        }
                    }
                    _ => {}
                }
            }
        }
        Node::Value(value) => {
            let span = value.span()?;
            found.push(Piece {
                range: line_start(text, span.start)..line_end(text, span.end),
                header: false,
            });
        }
        _ => return None,
    }
    Some(())
}

/// 有表头的表 `table`：表头那一行到它最后一个值（连同点号连着写在里面的）那一行。
fn header(text: &str, table: &Table) -> Option<Piece> {
    let span = table.span()?;
    let end = last_value_end(table).unwrap_or(span.end).max(span.end);
    Some(Piece {
        range: line_start(text, span.start)..line_end(text, end),
        header: true,
    })
}

/// 这张表里（连同点号连着写在它里面的）最后一个值在第几个字节结束。
fn last_value_end(table: &Table) -> Option<usize> {
    table
        .iter()
        .filter_map(|(_, node)| match node {
            Node::Value(value) => value.span().map(|span| span.end),
            Node::Table(inner) if inner.is_dotted() => last_value_end(inner),
            _ => None,
        })
        .max()
}

/// 删掉 `found` 这几段（照先后排好的），后面的先删：前面的位置不跟着挪。
fn cut(text: &str, found: &[Piece]) -> String {
    let mut out = text.to_string();
    for piece in found.iter().rev() {
        out.replace_range(piece.range.clone(), "");
    }
    out
}

/// 接在末尾：前面不是空的，空一行。
fn appended(text: &str, block: &str, nl: &str) -> String {
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(nl);
    }
    if !out.trim().is_empty() && !out.ends_with(&format!("{nl}{nl}")) {
        out.push_str(nl);
    }
    out.push_str(block);
    out
}

/// 从第 `at` 个字节（一行的开头）往后跳过空行，交回第一个不空的行从哪开始。
fn blank_after(text: &str, mut at: usize) -> usize {
    while at < text.len() && text[at..line_end(text, at)].trim().is_empty() {
        at = line_end(text, at);
    }
    at
}

/// 这份字用的换行：第一个换行是 `\r\n` 的用它，别的用 `\n`。
fn newline(text: &str) -> &'static str {
    match text.find('\n') {
        Some(at) if at > 0 && text.as_bytes()[at - 1] == b'\r' => "\r\n",
        _ => "\n",
    }
}

/// 第 `at` 个字节所在的那一行从哪开始。
fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |n| n + 1)
}

/// 第 `at` 个字节所在的那一行在哪结束（连同换行）。
fn line_end(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |n| at + n + 1)
}

#[cfg(test)]
mod tests;
