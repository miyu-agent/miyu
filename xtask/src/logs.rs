//! 日志门禁（施工 LG-1，设计 `28-运行日志.md` LG3、`docs/blueprint/log.md` 第七节）：运行日志里不写对话的正文、工具的参数和
//! 结果、路径，只写编号、长度、类别。管理员读得到运行日志，成员的会话不能从这里漏出去（设计 06 U2）。
//!
//! 逐个包扫 `src/`（测试不扫）里的 `tracing::{trace,debug,info,warn,error}!` 和 `*_span!`：
//!
//! 1. 格名不许是路径、正文、名字这一类（[`FIELDS`]）；`host` 只许在 [`ALLOWED`] 里写明的地方。
//! 2. 值里不许 `.display()`、`to_string_lossy()`、`as_os_str()`、`format!(`：都是把路径、拼出来的字写进去。
//! 3. 消息不许插值（带 `{`，或者消息后面还跟着参数）：要说是哪一种，用一格写。
//! 4. 消息里说「panicked」「task failed」的不许带 `error`：任务的 `JoinError` 照 Display 写会把 panic 的原话整句写进去，写
//!    `panic = error.is_panic()`。不管消息怎么说，`match` 里先有 `Ok(Ok(…))`、`Ok(Err(…))`、后面 `Err(e) =>` 那一支（等
//!    后台任务那一层出的错）写了 `%e`、`?e` 的也一样。
//!
//! 写明了为什么可以的放在 [`ALLOWED`]；还没改完的（施工 LG-1 下清空）放在 [`PENDING`]。按文本找，认字符串：`https://` 里的
//! `//` 不当注释。它防的是手滑，不是存心绕过。

/// 不许的格名：路径、正文、人起的名字、外部身份。
const FIELDS: &[&str] = &[
    "path",
    "file",
    "dir",
    "cwd",
    "workspace",
    "root",
    "home",
    "helper",
    "socket",
    "program",
    "url",
    "base",
    "link",
    "site",
    "host",
    "text",
    "prompt",
    "message",
    "content",
    "body",
    "args",
    "arguments",
    "query",
    "title",
    "name",
    "value",
    "detail",
    "external",
    "room",
    "kb",
    "persona",
    "preset",
    "secret",
];

/// 写明了为什么可以的：文件（相对仓库根，照 `/` 写）、格名、为什么。
pub const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "crates/miyu-http/src/send.rs",
        "host",
        "模型供应商的主机：管理员配的，排查连不上要它（LG-1 主会话定）",
    ),
    (
        "crates/miyu-log/src/lib.rs",
        "dir",
        "日志自己的目录：数据根里的 `state/logs`，不是谁的东西",
    ),
    (
        "crates/miyu-session/src/route/shared.rs",
        "file",
        "`state/models/` 下写死的几个文件名（`learned.json` 这种），不是谁的东西",
    ),
    (
        "crates/miyu-session/src/route/shared/recall.rs",
        "file",
        "`state/models/catalog.json`，写死的文件名，不是谁的东西（施工 V-25 下从 `shared.rs` 拆出来）",
    ),
    (
        "crates/miyu-session/src/embed/files.rs",
        "file",
        "嵌入模型清单里的文件名：随安装来的，不是谁的东西",
    ),
    (
        "crates/miyu-endpoint/src/mcp/prompts.rs",
        "prompt",
        "MCP 服务交的提示词名：同工具名、服务名，装上的东西照写（LG-1 主会话定）",
    ),
];

/// 消息里插的是写死的字（几种固定的用途、写死的一句）的文件：插值照放，写明为什么。
pub const FIXED_MESSAGES: &[(&str, &str)] = &[
    (
        "crates/miyu-endpoint/src/hello.rs",
        "握手拒绝的那一句照凭据的种类挑，都是写死的字",
    ),
    (
        "crates/miyu-endpoint/src/login/files.rs",
        "`what` 是 `&'static str`，写的是哪一份文件没写成",
    ),
    (
        "crates/miyu-session/src/actor/model.rs",
        "请求的用途（`recap`、`title`、`compaction` 这些）是写死的几种",
    ),
];

pub use pending::PENDING;

/// 扫一个文件。`label` 是相对仓库根的路径（照 `/` 写）。测试文件、测试模块不扫。
pub fn scan(label: &str, source: &str) -> Vec<String> {
    if is_test_file(label) {
        return Vec::new();
    }
    let code = without_tests(source);
    let mut found = Vec::new();
    for call in calls(&code) {
        let line = line_of(&code, call.at);
        let joined = joined(&code, call.at, &call.args);
        for problem in judge(label, &call.args, call.span)
            .into_iter()
            .chain(joined)
        {
            found.push((line, format!("{label}:{line}：{problem}（LG-1）")));
        }
    }
    found.sort();
    found.into_iter().map(|(_, problem)| problem).collect()
}

/// 等后台任务那一层出的错（`JoinError`）：`match` 里先有 `Ok(Ok(…))`、`Ok(Err(…))` 的分支，后面 `Err(e) =>` 那一支的
/// 日志写了 `%e`、`?e` 的，panic 的内容整句进日志。只许 `panic = e.is_panic()`。
fn joined(code: &str, at: usize, args: &str) -> Option<String> {
    let lines: Vec<&str> = code[..at].split('\n').collect();
    let here = lines.len() - 1;
    let (arm, name) = (here.saturating_sub(3)..=here).rev().find_map(|index| {
        let rest = lines[index].trim_start().strip_prefix("Err(")?;
        let name = &rest[..rest.find(')')?];
        let ident = !name.is_empty() && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric());
        (ident && rest[name.len()..].starts_with(") =>")).then_some((index, name))
    })?;
    let shown = split_top(args).iter().any(|part| {
        let part = part.trim();
        part.ends_with(&format!("%{name}")) || part.ends_with(&format!("?{name}"))
    });
    if !shown {
        return None;
    }
    let indent = lines[arm].len() - lines[arm].trim_start().len();
    for above in lines[..arm].iter().rev().take(40) {
        let trimmed = above.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let depth = above.len() - trimmed.len();
        if depth < indent {
            return None;
        }
        if depth == indent && (trimmed.starts_with("Ok(Ok(") || trimmed.starts_with("Ok(Err(")) {
            return Some(format!(
                "`Err({name})` 是等后台任务出的错，`{name}` 原话会带 panic 的内容：只写 `panic = {name}.is_panic()`"
            ));
        }
    }
    None
}

/// 测试用的文件：`tests.rs`、`tests/` 下的、`test_support.rs`、`testkit`。
fn is_test_file(label: &str) -> bool {
    label.ends_with("/tests.rs")
        || label.contains("/tests/")
        || label.ends_with("test_support.rs")
        || label.contains("testkit")
}

/// 一处日志调用：在源码里的位置（宏名的开头），括号里的那一段，是不是 span（第一个字符串是名字，不是消息）。
struct Call {
    at: usize,
    args: String,
    span: bool,
}

/// 宏名。
const MACROS: &[&str] = &[
    "tracing::trace!(",
    "tracing::debug!(",
    "tracing::info!(",
    "tracing::warn!(",
    "tracing::error!(",
    "tracing::trace_span!(",
    "tracing::debug_span!(",
    "tracing::info_span!(",
    "tracing::warn_span!(",
    "tracing::error_span!(",
];

/// 源码里的每一处日志调用：括号照认字符串的办法配对。宏名在字符串里的（例如这张表自己）不算。
fn calls(code: &str) -> Vec<Call> {
    let masked = masked(code);
    let mut out = Vec::new();
    for name in MACROS {
        let mut from = 0;
        while let Some(found) = masked[from..].find(name) {
            let at = from + found;
            let open = at + name.len() - 1;
            if let Some(close) = closing(code, open) {
                out.push(Call {
                    at,
                    args: code[open + 1..close].to_string(),
                    span: name.contains("_span!"),
                });
            }
            from = open + 1;
        }
    }
    out.sort_by_key(|call| call.at);
    out
}

/// 字符串的内容换成空白（引号留着），长度、行数不变：找宏名时字符串里的不算。
fn masked(code: &str) -> String {
    let bytes = code.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let end = skip_string(bytes, i).unwrap_or(bytes.len() - 1);
            for byte in &mut out[i + 1..end] {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
            i = end;
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// 从 `open`（一个 `(`）起配对，交回配上的 `)` 的位置。字符串、字符里的括号不算。
fn closing(code: &str, open: usize) -> Option<usize> {
    let bytes = code.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = skip_string(bytes, i)?,
            b'\'' => i = skip_char(bytes, i),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// 跳过从 `start`（`"`）起的字符串，交回收尾的 `"` 的位置。前面紧跟 `r#…` 的照原始字符串认。
fn skip_string(bytes: &[u8], start: usize) -> Option<usize> {
    let hashes = raw_hashes(bytes, start);
    let mut i = start + 1;
    while i < bytes.len() {
        match (hashes, bytes[i]) {
            (None, b'\\') => i += 1,
            (None, b'"') => return Some(i),
            (Some(n), b'"')
                if bytes[i + 1..]
                    .iter()
                    .take(n)
                    .filter(|b| **b == b'#')
                    .count()
                    == n =>
            {
                return Some(i + n);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// `start` 上的 `"` 是不是原始字符串的开头：是的交回 `#` 的个数。
fn raw_hashes(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    let mut hashes = 0;
    while i > 0 && bytes[i - 1] == b'#' {
        hashes += 1;
        i -= 1;
    }
    (i > 0 && bytes[i - 1] == b'r').then_some(hashes)
}

/// 跳过字符字面量（`'('`、`'\''`）；生命周期（`'a`）照原样往下。
fn skip_char(bytes: &[u8], start: usize) -> usize {
    match (
        bytes.get(start + 1),
        bytes.get(start + 2),
        bytes.get(start + 3),
    ) {
        (Some(b'\\'), _, _) => bytes[start + 2..]
            .iter()
            .position(|b| *b == b'\'')
            .map_or(start, |end| start + 2 + end),
        (Some(_), Some(b'\''), _) => start + 2,
        _ => start,
    }
}

/// 判一处调用的参数：交回每一样不许的。span 的第一个字符串是名字，前后都是格。
fn judge(label: &str, args: &str, span: bool) -> Vec<String> {
    let parts = split_top(args);
    let mut problems = Vec::new();
    let message = parts
        .iter()
        .position(|part| part.trim_start().starts_with('"'));
    let (mut fields, message, after) = match message {
        Some(at) => (
            parts[..at].to_vec(),
            Some(parts[at].trim()),
            &parts[at + 1..],
        ),
        None => (parts.clone(), None, &parts[0..0]),
    };
    let message = if span {
        fields.extend(after.iter().cloned());
        None
    } else {
        message
    };
    for field in &fields {
        let field = field.trim();
        if field.is_empty() || field.starts_with("target") || field.starts_with("parent") {
            continue;
        }
        let (name, value) = match field.split_once('=') {
            Some((name, value)) => (name.trim(), value.trim()),
            None => (field.trim_start_matches(['%', '?']).trim(), field),
        };
        let name = name.trim_start_matches(['%', '?']);
        let last = name.rsplit('.').next().unwrap_or(name);
        if FIELDS.contains(&last) && !allowed(label, last) {
            problems.push(format!(
                "格名 `{name}` 是路径、正文或人起的名字这一类，不写进运行日志"
            ));
        }
        for leak in [".display()", "to_string_lossy", "as_os_str", "format!("] {
            if value.contains(leak) && !allowed(label, last) {
                problems.push(format!(
                    "`{name}` 的值用了 `{leak}`：路径、拼出来的字不写进运行日志"
                ));
            }
        }
        if name == "error"
            && message.is_some_and(|message| {
                message.contains("panicked") || message.contains("task failed")
            })
        {
            problems.push(
                "说 panicked、task failed 的不带 `error`：任务的错误照 Display 写会把 panic 的原话写进去，写 `panic = error.is_panic()`"
                    .to_string(),
            );
        }
    }
    if let Some(message) = message
        && (message.contains('{') || !after.iter().all(|part| part.trim().is_empty()))
        && !FIXED_MESSAGES.iter().any(|(file, _)| label == *file)
    {
        problems.push("消息里插了值：要说是哪一种，用一格写".to_string());
    }
    problems
}

/// 这个文件的这一格写明了为什么可以，或者还没改完（LG-1 下）。
fn allowed(label: &str, field: &str) -> bool {
    ALLOWED
        .iter()
        .any(|(file, name, _)| label == *file && field == *name)
        || PENDING
            .iter()
            .any(|(file, name)| label == *file && field == *name)
}

/// 照最外层的逗号切开（字符串、括号里的不算）。
fn split_top(args: &str) -> Vec<String> {
    let bytes = args.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = skip_string(bytes, i).unwrap_or(bytes.len()),
            b'\'' => i = skip_char(bytes, i),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                parts.push(args[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(args[start..].to_string());
    parts
}

/// 去掉 `#[cfg(test)]` 的模块（括号里的整个换成空白，行数不变）和注释。
fn without_tests(source: &str) -> String {
    let code = without_comments(source);
    let mut out = code.clone();
    let mut from = 0;
    while let Some(found) = code[from..].find("#[cfg(test)]") {
        let at = from + found;
        let rest = &code[at..];
        let Some(open) = rest.find('{') else { break };
        let is_module = rest[..open].contains("mod ");
        if is_module && let Some(close) = closing(&code, at + open) {
            blank(&mut out, at, close + 1);
            from = close + 1;
        } else {
            from = at + 1;
        }
    }
    out
}

/// 把 `[from, to)` 换成空白，换行留着。
fn blank(text: &mut String, from: usize, to: usize) {
    let replaced: String = text[from..to]
        .chars()
        .map(|c| if c == '\n' { '\n' } else { ' ' })
        .collect();
    text.replace_range(from..to, &replaced);
}

/// 去掉每一行 `//` 之后的注释：字符串里的 `//` 不算。行数不变。
fn without_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let mut start = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = skip_string(bytes, i).unwrap_or(bytes.len() - 1),
            b'\'' => i = skip_char(bytes, i),
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                out.push_str(&source[start..i]);
                let end = source[i..].find('\n').map_or(bytes.len(), |end| i + end);
                out.push_str(&" ".repeat(end - i));
                i = end;
                start = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out.push_str(&source[start.min(source.len())..]);
    out
}

/// `at` 在第几行（从 1 数）。
fn line_of(code: &str, at: usize) -> usize {
    code[..at].matches('\n').count() + 1
}

mod pending;

#[cfg(test)]
mod tests;
