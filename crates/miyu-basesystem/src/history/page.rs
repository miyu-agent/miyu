//! 一页怎么写（`docs/blueprint/tools/history.md`「怎么走」第 4、5 条，施工 6-4）：「找」一条一行，新的在前；「读」照
//! 先后，一条头一行、下面是原文。整页最多 [`PAGE`] 个字，和 `shell` 一样。

use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;

use super::entry::Entry;
use crate::load::say;

/// 一页最多多少个字（Unicode 字符）。
pub(super) const PAGE: usize = 30_000;
/// 「找」摘出来的一段最多多少个字。
const SNIPPET: usize = 200;

/// 往下翻的两句、一条太长截掉的那一句。
#[derive(Clone)]
pub(super) struct Footers {
    /// `history/more-found.txt`：`shown`、`total`、`next`。
    pub(super) more_found: Template,
    /// `history/more-read.txt`：`first`、`last`、`next`。
    pub(super) more_read: Template,
    /// `history/cut.txt`：`shown`、`total`。
    pub(super) cut: Template,
}

/// 写好的一页：给她看的，和这一页里第一条、最后一条的序号，一共几条（「找」的是命中几条；「读」的是交进来的 `total`，
/// 施工 V-45 起只够认有没有）。
pub(super) struct Page {
    pub(super) text: String,
    pub(super) first: u64,
    pub(super) last: u64,
    pub(super) total: usize,
}

/// 「找」：`found` 是命中的最新几条（一页可能放得下的都在，施工 V-45 边读边扔），照先后；每条带着摘出来的一段（[`snippet`]）。
/// `total` 是一共命中几条。新的在前，一页最多 `limit` 条。
pub(super) fn found(
    found: &[(Entry, String)],
    total: usize,
    limit: usize,
    offset: UtcOffset,
    footers: &Footers,
) -> Page {
    let mut text = String::new();
    let mut chars = 0;
    let mut shown: Vec<u64> = Vec::new();
    for (entry, snippet) in found.iter().rev().take(limit) {
        let line = format!(
            "#{} {} {}: {snippet}\n",
            entry.seq,
            entry.at.local_minute(offset),
            entry.said_by(),
        );
        let size = line.chars().count();
        if !shown.is_empty() && chars + size > PAGE {
            break;
        }
        chars += size;
        text.push_str(&line);
        shown.push(entry.seq);
    }
    let (first, last) = (*shown.last().unwrap_or(&0), *shown.first().unwrap_or(&0));
    if shown.len() < total {
        text.push_str(&say(
            &footers.more_found,
            &[
                ("shown", &shown.len().to_string()),
                ("total", &total.to_string()),
                ("next", &first.saturating_sub(1).to_string()),
            ],
        ));
    }
    Page {
        text,
        first,
        last,
        total,
    }
}

/// 「读」：照先后，从第一条起，最多 `limit` 条，整页不超过 [`PAGE`] 个字。第一条就超过的，截到放得下。`entries` 是从第一条起
/// 一页可能放得下的几条（施工 V-45 边读边扔），`total` 是筛得进的一共几条（只用来认这一页后面还有没有）。
pub(super) fn read(
    entries: &[Entry],
    total: usize,
    limit: usize,
    offset: UtcOffset,
    footers: &Footers,
) -> Page {
    let mut text = String::new();
    let mut chars = 0;
    let mut shown: usize = 0;
    for entry in entries.iter().take(limit) {
        let head = format!(
            "#{} {} {}\n",
            entry.seq,
            entry.at.local_minute(offset),
            entry.said_by()
        );
        let gap = if shown == 0 { "" } else { "\n" };
        let size = gap.len() + head.chars().count() + entry.text.chars().count() + 1;
        if shown > 0 && chars + size > PAGE {
            break;
        }
        text.push_str(gap);
        text.push_str(&head);
        if size > PAGE {
            let room = PAGE.saturating_sub(head.chars().count() + 1);
            let total = entry.text.chars().count();
            text.extend(entry.text.chars().take(room));
            text.push('\n');
            text.push_str(&say(
                &footers.cut,
                &[("shown", &room.to_string()), ("total", &total.to_string())],
            ));
            shown += 1;
            break;
        }
        text.push_str(&entry.text);
        text.push('\n');
        chars += size;
        shown += 1;
    }
    let first = entries.first().map_or(0, |entry| entry.seq);
    let last = entries
        .get(shown.saturating_sub(1))
        .map_or(0, |entry| entry.seq);
    if shown < total {
        text.push_str(&say(
            &footers.more_read,
            &[
                ("first", &first.to_string()),
                ("last", &last.to_string()),
                ("next", &(last + 1).to_string()),
            ],
        ));
    }
    Page {
        text,
        first,
        last,
        total,
    }
}

/// 一条命中的：空白收过的原文、第一处命中从第几个字起、命中的词多少个字。
pub(super) struct Hit {
    pub(super) text: String,
    pub(super) at: usize,
    pub(super) len: usize,
}

/// `text` 里每个词（已经转成小写）都有的，交回第一处命中；不分大小写，照原样比，中文不分词。比之前连着的空白
/// （换行也算）收成一个空格。
///
/// 每一条都要比（施工 V-45 量的大会话里一次两万多条），所以不另攒「小写的每个字来自原文第几个字」的整张表（一个字 8 字节）：
/// 只在找到的那一处现数（[`origin`]），原文全是 ASCII 的转小写一字对一字，直接就是。
pub(super) fn hit(text: &str, words: &[String]) -> Option<Hit> {
    let text = collapse(text);
    let folded = fold(&text);
    let mut first: Option<(usize, usize)> = None;
    for word in words {
        let byte = folded.find(word.as_str())?;
        let at = origin(&text, byte);
        let len = word.chars().count();
        if first.is_none_or(|(earliest, _)| at < earliest) {
            first = Some((at, len));
        }
    }
    let (at, len) = first?;
    Some(Hit { text, at, len })
}

/// 连着的空白收成一个空格，头尾的去掉。
fn collapse(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !collapsed.is_empty() {
            collapsed.push(' ');
        }
        collapsed.push_str(word);
    }
    collapsed
}

/// 一个字一个字转成小写（有的字转成小写不止一个）。不用 `str::to_lowercase`：它照上下文转希腊字母 Σ，和一个字一个字转的不一样。
fn fold(text: &str) -> String {
    if text.is_ascii() {
        return text.to_ascii_lowercase();
    }
    let mut folded = String::with_capacity(text.len());
    for c in text.chars() {
        folded.extend(c.to_lowercase());
    }
    folded
}

/// [`fold`] 转出来的第 `byte` 个字节起的那个字，来自原文 `text` 的第几个字。
fn origin(text: &str, byte: usize) -> usize {
    if text.is_ascii() {
        return byte;
    }
    let mut end = 0;
    for (k, c) in text.chars().enumerate() {
        end += c.to_lowercase().map(char::len_utf8).sum::<usize>();
        if end > byte {
            return k;
        }
    }
    text.chars().count()
}

/// 摘第 `at` 个字起、长 `len` 个字的命中前后一段，一共最多 [`SNIPPET`] 个字；前后截掉了的写 `…`。照字的位置切原文，不把整段
/// 拆成一个个字另存（施工 V-45：每一条命中都要摘）。
pub(super) fn snippet(text: &str, at: usize, len: usize) -> String {
    let count = text.chars().count();
    if count <= SNIPPET {
        return text.to_string();
    }
    let before = SNIPPET.saturating_sub(len) / 2;
    let start = at.saturating_sub(before).min(count - SNIPPET);
    let end = start + SNIPPET;
    let byte = |k: usize| {
        text.char_indices()
            .nth(k)
            .map_or(text.len(), |(byte, _)| byte)
    };
    let mut out = String::with_capacity(SNIPPET * 4 + 6);
    if start > 0 {
        out.push('…');
    }
    out.push_str(&text[byte(start)..byte(end)]);
    if end < count {
        out.push('…');
    }
    out
}
