//! 写 Markdown 的几样小事：块之间空一行；正文原样，没合上的代码块替它合上；代码块、行内代码的反引号比里面的多；折叠块
//! `<details>` 里的正文不让一行 `</details>` 提前合上。

use std::borrow::Cow;

/// 写着的 Markdown：一块一块往后接，块之间空一行。
#[derive(Debug, Default)]
pub(super) struct Out {
    text: String,
    /// 最后接的是列表里的一行（[`Out::item`]）：下一行接着写，不空行。
    listing: bool,
}

impl Out {
    /// 接一块：前面有字的先空一行；块末尾的空白去掉。
    pub(super) fn block(&mut self, block: &str) {
        let block = block.trim_end();
        if block.is_empty() {
            return;
        }
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.push_str(block);
        self.listing = false;
    }

    /// 接列表里的一行：上一块也是列表里的一行的紧接着写（同一个列表），不然另起一块。
    pub(super) fn item(&mut self, line: &str) {
        let line = line.trim_end();
        if self.listing {
            self.text.push('\n');
            self.text.push_str(line);
        } else {
            self.block(line);
        }
        self.listing = true;
    }

    /// 写完：以一个换行结尾。
    pub(super) fn finish(mut self) -> String {
        self.text.push('\n');
        self.text
    }
}

/// 一行里放得下的样子：换行、回车换成空格，前后空白去掉。
pub(super) fn one_line(text: &str) -> String {
    text.split(['\n', '\r'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 一行旁白：引用块里的一行。
pub(super) fn quote(text: &str) -> String {
    format!("> {}", one_line(text))
}

/// 一段正文原样：没合上的代码块在末尾替它合上，后面的标题、旁白才不会被吞进代码块。`in_details` 的（放进折叠块里的）另把
/// 代码块外面的 `</details` 写成 `&lt;/details`：一行 `</details>` 会把折叠块提前合上。
pub(super) fn prose(text: &str, in_details: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut open: Option<(char, usize)> = None;
    for line in text.split_inclusive('\n') {
        let fence = fence_of(line);
        match (open, fence) {
            (Some((mark, length)), Some((found, count, rest)))
                if found == mark && count >= length && rest.trim().is_empty() =>
            {
                open = None;
            }
            (None, Some((found, count, rest))) if found == '~' || !rest.contains('`') => {
                open = Some((found, count));
            }
            _ => {}
        }
        if in_details && open.is_none() && fence.is_none() {
            out.push_str(&escape_details(line));
        } else {
            out.push_str(line);
        }
    }
    if let Some((mark, length)) = open {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.extend(std::iter::repeat_n(mark, length));
    }
    out
}

/// 这一行是不是代码块的围栏：缩进最多三个空格，三个以上同样的反引号或波浪号。交回哪一种、几个、后面的字。
fn fence_of(line: &str) -> Option<(char, usize, &str)> {
    let body = line.trim_end_matches(['\n', '\r']);
    let indent = body.len() - body.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let body = &body[indent..];
    let mark = body.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let count = body.chars().take_while(|c| *c == mark).count();
    (count >= 3).then(|| (mark, count, &body[count..]))
}

/// 一行里的 `</details`（不分大小写）写成 `&lt;/details`。
fn escape_details(line: &str) -> Cow<'_, str> {
    const TAG: &str = "</details";
    if !line.to_ascii_lowercase().contains(TAG) {
        return Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len() + 8);
    let mut rest = line;
    while let Some(at) = rest.to_ascii_lowercase().find(TAG) {
        out.push_str(&rest[..at]);
        out.push_str("&lt;");
        out.push_str(&rest[at + 1..at + TAG.len()]);
        rest = &rest[at + TAG.len()..];
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// 一段字放进代码块：围栏的反引号比字里最长的一串多一个，至少三个。
pub(super) fn code_block(text: &str, info: &str) -> String {
    let fence = "`".repeat((longest_run(text) + 1).max(3));
    format!(
        "{fence}{info}\n{}\n{fence}",
        text.trim_end_matches(['\n', '\r'])
    )
}

/// 一行字写成行内代码：反引号比字里最长的一串多一个；字的头尾是反引号的两边垫一个空格。
pub(super) fn code_span(text: &str) -> String {
    let text = one_line(text);
    let ticks = "`".repeat(longest_run(&text) + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{ticks}{pad}{text}{pad}{ticks}")
}

/// 字里最长的一串反引号。
fn longest_run(text: &str) -> usize {
    text.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

/// 折叠块：`summary` 那一行照 HTML 转义，`body` 前后各空一行（GitHub 照它把里面当 Markdown）。
pub(super) fn details(summary: &str, body: &str) -> String {
    format!(
        "<details>\n<summary>{}</summary>\n\n{}\n\n</details>",
        html(&one_line(summary)),
        body.trim_end()
    )
}

/// HTML 里的 `&`、`<`、`>` 转义。
fn html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 长的一段截成头尾（`agent`、`chat` 写工具结果，施工 SE-1 中下中）：超过 `most` 个字的留头尾各一半，中间一行写省了几个字
/// （固定英文，同 `agent` 的开头）。头尾能退到换行的退到换行（退得不多于一半的一半），整行好读。
pub(super) fn head_and_tail(text: &str, most: usize) -> Cow<'_, str> {
    let count = text.chars().count();
    if count <= most {
        return Cow::Borrowed(text);
    }
    let half = most / 2;
    let byte = |n: usize| text.char_indices().nth(n).map_or(text.len(), |(at, _)| at);
    let mut head = byte(half);
    if let Some(at) = text[..head].rfind('\n').filter(|at| *at >= byte(half / 2)) {
        head = at;
    }
    let mut tail = byte(count - half);
    if let Some(at) = text[tail..].find('\n').map(|at| tail + at + 1)
        && at <= byte(count - half / 2)
    {
        tail = at;
    }
    let omitted = text[head..tail].chars().count();
    Cow::Owned(format!(
        "{}\n[… {omitted} characters omitted …]\n{}",
        text[..head].trim_end_matches('\n'),
        &text[tail..]
    ))
}
