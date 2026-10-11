//! 切块（施工 K-1 上，`docs/blueprint/knowledge.md`「怎么走」第二条）：把知识库的一篇文档切成一块块，每块带着它的标题路径。
//! 调研（`docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第五节第 7 条）：200～400 token 一块的递归切分就够好。
//!
//! - Markdown 照标题（`#` 到 `######`）切：一节一块，标题路径是上面各级标题照层级连起来；代码块（三个反引号或波浪线起止）里
//!   的 `#` 不算标题。
//! - 纯文本照段落：空行分段，连着的几段合成一块。
//! - 每块最多 [`LIMIT`] 字节：一节、一段太长的照段落再切，一段自己就超过的照字切（截在字的边界上）；切开的每块都带着同样
//!   的标题路径。只有空白的不要。

#[cfg(test)]
mod tests;

/// 一块最多几个字节（标题那一行算在里面）：约 300 token，中文为主照字节/4 折算会少算一些。
pub const LIMIT: usize = 1200;

/// 标题那一行最多几个字节：层级再深、标题再长，也给正文留出大半。
const HEADING: usize = 300;

/// 切出来的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// 标题路径（`语法 › て形`）；没有标题的是空的。
    pub heading: String,
    /// 这一块的字：有标题的头一行是 `# <标题路径>`，接着正文。
    pub text: String,
}

/// 切一篇 Markdown。
#[must_use]
pub fn markdown(source: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut path: Vec<(usize, String)> = Vec::new();
    let mut body: Vec<&str> = Vec::new();
    let mut fenced = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
        }
        match heading(line).filter(|_| !fenced) {
            Some((level, title)) => {
                section(&path, &body, &mut chunks);
                body.clear();
                while path.last().is_some_and(|(top, _)| *top >= level) {
                    path.pop();
                }
                path.push((level, title));
            }
            None => body.push(line),
        }
    }
    section(&path, &body, &mut chunks);
    chunks
}

/// 切一篇纯文本：没有标题，照段落。
#[must_use]
pub fn plain(source: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    section(&[], &lines, &mut chunks);
    chunks
}

/// 一行是不是标题：`#` 到 `######` 后面跟空白和字；交回层级和去掉前后 `#`、空白的标题。
fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let title = rest.trim().trim_end_matches('#').trim();
    (!title.is_empty()).then(|| (level, title.to_string()))
}

/// 一节的正文 `body`（标题路径 `path`）切成几块，接在 `chunks` 后面。
fn section(path: &[(usize, String)], body: &[&str], chunks: &mut Vec<Chunk>) {
    let heading = path
        .iter()
        .map(|(_, title)| title.as_str())
        .collect::<Vec<_>>()
        .join(" › ");
    let head = if heading.is_empty() {
        String::new()
    } else {
        format!("# {}\n", cut(&heading, HEADING))
    };
    let room = LIMIT - head.len();
    let text = body.join("\n");
    let mut current = String::new();
    for paragraph in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        for piece in pieces(paragraph, room) {
            let needed = if current.is_empty() {
                piece.len()
            } else {
                current.len() + 2 + piece.len()
            };
            if needed > room && !current.is_empty() {
                push(&heading, &head, &mut current, chunks);
            }
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(piece);
        }
    }
    push(&heading, &head, &mut current, chunks);
}

/// 收下攒着的这一块（空的不要），清空。
fn push(heading: &str, head: &str, current: &mut String, chunks: &mut Vec<Chunk>) {
    if current.trim().is_empty() {
        current.clear();
        return;
    }
    chunks.push(Chunk {
        heading: heading.to_string(),
        text: format!("{head}{current}"),
    });
    current.clear();
}

/// 一段照字切成不超过 `room` 字节的几截，截在字的边界上。
fn pieces(paragraph: &str, room: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = paragraph;
    while rest.len() > room {
        let piece = cut(rest, room);
        out.push(piece);
        rest = &rest[piece.len()..];
    }
    out.push(rest);
    out
}

/// `text` 的头至多 `room` 字节，截在字的边界上（至少一个字）。
fn cut(text: &str, room: usize) -> &str {
    let mut end = room.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 {
        end = text.chars().next().map_or(0, char::len_utf8);
    }
    &text[..end]
}
