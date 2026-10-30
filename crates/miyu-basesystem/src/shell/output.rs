//! 命令的输出（施工 4-8）：内存里只留头尾，边读边解成字推给头，结束以后截成头尾两段交给她。
//!
//! 给她看的最多 [`LIMIT`] 个字（照 Claude Code）。多了留头尾各一半：开头多半是它在做什么，结尾多半是出了什么错。
//! 能截在行尾就截在行尾。照 UTF-8 解，解不开的字节换成 U+FFFD；PowerShell 出错写的 CLIXML 换回纯文本（施工 SW-4，
//! [`super::clixml`]）；`\r\n` 换成 `\n`。
//!
//! 脚本工具的输出另去掉进度行（施工 X-45）：一整行以 [`PROGRESS`] 开头的只推给头，不进结果，行数、字数、截长都照去掉以后的算。

use std::collections::VecDeque;

#[cfg(test)]
mod tests;

/// 给她看的最多多少个字。
pub(super) const LIMIT: usize = 30_000;

/// 进度行的开头（施工 X-45）：照字面，分大小写，后面带一个空格。
const PROGRESS: &[u8] = b"miyu-progress: ";

/// 内存里留的头、尾各多少字节：头尾各给她看 [`LIMIT`] 的一半个字，一个字最多 4 个字节，留得下。
const KEEP: usize = 64 * 1024;

/// 读到的输出：头 [`KEEP`] 字节、尾 [`KEEP`] 字节，一共多少字节、多少个字、多少个换行。输出再多，内存也不涨。
#[derive(Debug, Default, Clone)]
pub(super) struct Capture {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    bytes: u64,
    chars: u64,
    breaks: u64,
    last: Option<u8>,
    /// 去进度行的（施工 X-45）：只有脚本工具的有。
    lines: Option<Line>,
}

/// 去进度行时读到哪了（施工 X-45）：一行的开头最多攒 [`PROGRESS`] 那么长，看出来不是的当场交出去，内存不涨。
#[derive(Debug, Clone)]
enum Line {
    /// 行首，还看不出是不是：攒着的开头。
    Head(Vec<u8>),
    /// 不是进度行：到换行为止都交出去。
    Keep,
    /// 是进度行：到换行为止都丢掉。
    Drop,
}

impl Line {
    /// 读到一段：交回进结果的那几个字节。
    fn pass(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut kept = Vec::with_capacity(chunk.len());
        for &byte in chunk {
            let ends = byte == b'\n';
            let next = match self {
                Line::Head(head) => {
                    head.push(byte);
                    if head.as_slice() == PROGRESS {
                        Some(Line::Drop)
                    } else if PROGRESS.starts_with(head) {
                        None
                    } else {
                        kept.append(head);
                        Some(if ends {
                            Line::Head(Vec::new())
                        } else {
                            Line::Keep
                        })
                    }
                }
                Line::Keep => {
                    kept.push(byte);
                    ends.then(|| Line::Head(Vec::new()))
                }
                Line::Drop => ends.then(|| Line::Head(Vec::new())),
            };
            if let Some(next) = next {
                *self = next;
            }
        }
        kept
    }
}

/// 截给她看的。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Shown {
    /// 没超过上限，整段。
    Whole(String),
    /// 超过了，只留头尾：中间省了多少个字，一共多少个字。
    Cut {
        /// 开头那一段。
        head: String,
        /// 结尾那一段。
        tail: String,
        /// 中间省掉的字数。
        omitted: u64,
        /// 一共多少个字。
        total: u64,
    },
}

impl Capture {
    /// 一份空的：`progress_lines` 说去不去掉进度行（施工 X-45，只有脚本工具去掉）。
    pub(super) fn new(progress_lines: bool) -> Capture {
        Capture {
            lines: progress_lines.then(|| Line::Head(Vec::new())),
            ..Capture::default()
        }
    }

    /// 读完了：还攒着的那几个字（一行的开头，还看不出是不是进度行）照一整行判，交回读好的这一份。
    pub(super) fn finished(mut self) -> Capture {
        if let Some(Line::Head(head)) = self.lines.take() {
            self.add(&head);
        }
        self
    }

    /// 又读到一段：去进度行的先滤一遍。
    pub(super) fn push(&mut self, chunk: &[u8]) {
        let kept = match &mut self.lines {
            Some(lines) => lines.pass(chunk),
            None => return self.add(chunk),
        };
        self.add(&kept);
    }

    /// 记下进结果的一段。
    fn add(&mut self, chunk: &[u8]) {
        self.bytes += chunk.len() as u64;
        // 不是接续字节的，就是一个字的开头。
        self.chars += chunk.iter().filter(|&&byte| byte & 0xC0 != 0x80).count() as u64;
        self.breaks += chunk.iter().filter(|&&byte| byte == b'\n').count() as u64;
        if let Some(&byte) = chunk.last() {
            self.last = Some(byte);
        }
        let room = KEEP.saturating_sub(self.head.len()).min(chunk.len());
        let (head, rest) = chunk.split_at(room);
        self.head.extend_from_slice(head);
        self.tail.extend(rest);
        let over = self.tail.len().saturating_sub(KEEP);
        self.tail.drain(..over);
    }

    /// 什么都没输出。
    pub(super) fn is_empty(&self) -> bool {
        self.bytes == 0
    }

    /// 一共几行：最后一行没有换行的也算一行。
    pub(super) fn lines(&self) -> u64 {
        self.breaks + u64::from(self.last.is_some_and(|byte| byte != b'\n'))
    }

    /// 截成给她看的。中间丢过的，头尾接起来一定超过上限（两边各 [`KEEP`] 字节，比 [`LIMIT`] 个字多），接缝落在
    /// 截掉的那一段里。
    pub(super) fn shown(&self) -> Shown {
        let mut kept = self.head.clone();
        kept.extend(self.tail.iter());
        let text = lossy(kept);
        // PowerShell 出错写的 CLIXML 换回纯文本（施工 SW-4）。
        let text = super::clixml::plain(&text).into_owned();
        if text.chars().count() <= LIMIT {
            return Shown::Whole(unix(&text));
        }
        let head = front(&text, LIMIT / 2);
        let tail = back(&text, LIMIT / 2);
        let shown = (head.chars().count() + tail.chars().count()) as u64;
        Shown::Cut {
            head: unix(head),
            tail: unix(tail),
            omitted: self.chars.saturating_sub(shown),
            total: self.chars,
        }
    }
}

/// 开头的 `most` 个字：最后一个换行落在后一半里的，截在它后面。
fn front(text: &str, most: usize) -> &str {
    let end = text
        .char_indices()
        .nth(most)
        .map_or(text.len(), |(at, _)| at);
    let part = &text[..end];
    match part.rfind('\n') {
        Some(at) if at >= part.len() / 2 => &part[..=at],
        _ => part,
    }
}

/// 结尾的 `most` 个字：开头已经在行首的照原样；不然第一个换行落在前一半里的，从它后面起。
fn back(text: &str, most: usize) -> &str {
    let skip = text.chars().count().saturating_sub(most);
    let start = text
        .char_indices()
        .nth(skip)
        .map_or(text.len(), |(at, _)| at);
    let part = &text[start..];
    // 开头已经在行首的（前一个字是换行），照原样：不然多丢一整行（施工 4-9 再补二）。
    if start == 0 || text[..start].ends_with('\n') {
        return part;
    }
    match part.find('\n') {
        Some(at) if at < part.len() / 2 => &part[at + 1..],
        _ => part,
    }
}

/// 照 UTF-8 解，解不开的字节换成 U+FFFD。
fn lossy(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

/// `\r\n` 换成 `\n`。
fn unix(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// 边读边解：一个字被切在两段中间的，留到下一段再解。
#[derive(Debug, Default)]
pub(super) struct Decoder(Vec<u8>);

impl Decoder {
    /// 又读到一段：交回解得出的字。
    pub(super) fn push(&mut self, chunk: &[u8]) -> String {
        self.0.extend_from_slice(chunk);
        let ready = self.0.len() - unfinished(&self.0);
        lossy(self.0.drain(..ready).collect())
    }

    /// 读完了：剩下的也交回，解不开的换成 U+FFFD。
    pub(super) fn finish(&mut self) -> String {
        lossy(std::mem::take(&mut self.0))
    }
}

/// 边读边把 `\r\n` 换成 `\n`（施工 7-3，后台命令的输出一段段写进文件）：一段以 `\r` 结尾的，留着它，看下一段是不是
/// 以 `\n` 开头。单独的 `\r` 不动，和截给她看的一样。
#[derive(Debug, Default)]
pub(super) struct Crlf(bool);

impl Crlf {
    /// 又解出一段字：交回换好的。
    pub(super) fn push(&mut self, text: &str) -> String {
        let mut text = match std::mem::take(&mut self.0) {
            true => format!("\r{text}"),
            false => text.to_string(),
        };
        if text.ends_with('\r') {
            text.pop();
            self.0 = true;
        }
        unix(&text)
    }

    /// 读完了：留着的 `\r` 交回来。
    pub(super) fn finish(&mut self) -> String {
        match std::mem::take(&mut self.0) {
            true => "\r".to_string(),
            false => String::new(),
        }
    }
}

/// 末尾有几个字节是一个还没读完的字。
fn unfinished(bytes: &[u8]) -> usize {
    for back in 1..=bytes.len().min(3) {
        let byte = bytes[bytes.len() - back];
        if byte & 0xC0 != 0x80 {
            let needs = match byte {
                0xC0..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF7 => 4,
                _ => 1,
            };
            return if needs > back { back } else { 0 };
        }
    }
    0
}
