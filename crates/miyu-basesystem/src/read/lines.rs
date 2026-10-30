//! 按行读一份文本文件（`10-自带软件.md` 第三节）：认 UTF-8 和带 BOM 的 UTF-16，前 8 KiB 里有 NUL 字节的当
//! 二进制；每行是行号、一个制表符、原文，行号前不补空格（Claude Code 现在的写法，施工 4-4 下）；一行最长 2000
//! 个字，一次最多 64 KiB。
//!
//! 读的同一遍里算出整份文件的内容哈希（施工 4-6 上）：她改之前照它核对。不另读一遍，也不把整份读进内存：一行只留开头
//! [`LINE_BYTES`] 个字节，带 BOM 的 UTF-16 边读边解（审计 E-3，2026-10-30：以前整行整行、整份 UTF-16 读进来，读一份 64 MiB
//! 没有换行的文本要多分 133 MiB）；不在这一页的行只数换行，不拷不解（施工 V-31）。叫停了就不往下读。

use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read};

use miyu_kernel::id::{ContentHash, Hasher};
use miyu_tool::Stop;

use crate::common::OUTPUT_BYTES;

/// 看前多少个字节认编码、认二进制。
const SNIFF: usize = 8 * 1024;
/// 一行最长多少个字，多的截掉、补一个 `…`。
pub(crate) const LINE_CHARS: usize = 2000;

/// 一行留开头多少个字节（审计 E-3）：一个字最多 4 个字节，留够 [`LINE_CHARS`] 再多一个字。比它长的一行，留下的这一截照
/// 不严格的 UTF-8 解出来至少有 `LINE_CHARS + 1` 个字（坏字节一个也算一个字），前 `LINE_CHARS` 个和整行解出来的一样，截出来
/// 和整行截的一字不差；行尾的 `\r` 也只在整行都留下了时才要去掉。
const LINE_BYTES: usize = (LINE_CHARS + 1) * 4;

/// 读下来的一页。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Page {
    /// 二进制文件，不读内容。
    Binary,
    /// 空文件。
    Empty,
    /// `offset` 过了结尾：一共几行。
    PastEnd { total: u64 },
    /// 读到的几行：带行号的字，第几行到第几行（从 1 数起，含两头），一共几行。
    Lines {
        text: String,
        from: u64,
        to: u64,
        total: u64,
    },
}

/// 读下来的：这一页，和整份文件的内容哈希。二进制文件不读内容，哈希照样有（施工 4-9 再补二）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Paged {
    /// 这一页。
    pub(crate) page: Page,
    /// 整份文件的内容哈希，读了一段的也是整份的。
    pub(crate) hash: Option<ContentHash>,
}

/// 从第 `offset` 行起（从 1 数起），最多读 `limit` 行。叫停了（`stop`）不读下去，交回 `Interrupted`。
pub(crate) fn read(mut file: File, offset: u64, limit: u64, stop: &Stop) -> io::Result<Paged> {
    let mut head = Vec::with_capacity(SNIFF);
    (&mut file).take(SNIFF as u64).read_to_end(&mut head)?;
    match head.as_slice() {
        [0xFF, 0xFE, ..] => return utf16(head, file, u16::from_le_bytes, (offset, limit), stop),
        [0xFE, 0xFF, ..] => return utf16(head, file, u16::from_be_bytes, (offset, limit), stop),
        _ => {}
    }
    if head.contains(&0) {
        // 二进制的不读内容，照样过一遍整份算哈希：她读过它，`write` 才盖得了（施工 4-9 再补二）。
        let mut hasher = Hasher::default();
        hasher.update(&head);
        let mut rest = Hashing {
            inner: file,
            hasher,
        };
        io::copy(&mut rest, &mut io::sink())?;
        return Ok(Paged {
            page: Page::Binary,
            hash: Some(rest.hasher.finish()),
        });
    }
    let bom = if head.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    // 开头认编码读过的那一段，连同 BOM 先喂给哈希；往后从文件里读出来的，读一段喂一段。
    let mut hasher = Hasher::default();
    hasher.update(&head);
    let mut start = Cursor::new(head);
    start.set_position(bom);
    let mut reader = BufReader::new(start.chain(Hashing {
        inner: file,
        hasher,
    }));
    // 数一共几行要读到结尾，所以这一页数完了，整份都过了一遍哈希。
    let page = page(Lines::new(&mut reader, stop), offset, limit)?;
    let (_, rest) = reader.into_inner().into_inner();
    Ok(Paged {
        page,
        hash: Some(rest.hasher.finish()),
    })
}

/// 一行一行读：每行只留开头 [`LINE_BYTES`] 个字节，照不严格的 UTF-8 解成字（审计 E-3）。切行同 `BufRead::split`：以换行
/// 结尾的，最后不多出一行空的。行尾的 `\r` 去掉。不在这一页的行只往后数到换行（[`Source::skip`]），不拷、不解（施工 V-31：
/// 以前每一行都拷进新的 `Vec`、解成新的字串，读 8 MB 文件的末尾，这一项约占四成）。
struct Lines<'a, R> {
    reader: R,
    stop: &'a Stop,
    done: bool,
}

/// 一行一行交给 [`page`] 的：要的一行交字，不要的只数。
trait Source {
    /// 下一行的字；读完了是 `None`。
    fn line(&mut self) -> io::Result<Option<String>>;
    /// 跳过下一行，交回有没有这一行。
    fn skip(&mut self) -> io::Result<bool>;
}

impl<'a, R: BufRead> Lines<'a, R> {
    fn new(reader: R, stop: &'a Stop) -> Lines<'a, R> {
        Lines {
            reader,
            stop,
            done: false,
        }
    }

    /// 读过下一行：`kept` 是 `Some` 的，把这一行开头最多 [`LINE_BYTES`] 个字节接进去，交回这一行是不是比留下的长；`None`
    /// 的只往后数。读完了（没有下一行）交 `None`。
    fn next_line(&mut self, mut kept: Option<&mut Vec<u8>>) -> io::Result<Option<bool>> {
        let mut longer = false;
        let mut any = false;
        loop {
            if self.stop.stopped() {
                self.done = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            let buffer = match self.reader.fill_buf() {
                Ok(buffer) => buffer,
                Err(error) => {
                    self.done = true;
                    return Err(error);
                }
            };
            if buffer.is_empty() {
                self.done = true;
                return Ok(any.then_some(longer));
            }
            any = true;
            // 找换行用 memchr：一次比一整段，比一个字节一个字节地找快（V-29 量过，约占读大文件的 25%）。
            let (piece, used, ended) = match memchr::memchr(b'\n', buffer) {
                Some(at) => (&buffer[..at], at + 1, true),
                None => (buffer, buffer.len(), false),
            };
            if let Some(kept) = kept.as_deref_mut() {
                let room = LINE_BYTES - kept.len();
                kept.extend_from_slice(&piece[..piece.len().min(room)]);
                longer |= piece.len() > room;
            }
            self.reader.consume(used);
            if ended {
                return Ok(Some(longer));
            }
        }
    }
}

impl<R: BufRead> Source for Lines<'_, R> {
    fn line(&mut self) -> io::Result<Option<String>> {
        if self.done {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        let Some(longer) = self.next_line(Some(&mut bytes))? else {
            return Ok(None);
        };
        if !longer && bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
    }

    fn skip(&mut self) -> io::Result<bool> {
        if self.done {
            return Ok(false);
        }
        Ok(self.next_line(None)?.is_some())
    }
}

/// 读过去的字节都喂给哈希。
struct Hashing {
    inner: File,
    hasher: Hasher,
}

impl Read for Hashing {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
}

/// 带 BOM 的 UTF-16：边读边解成 UTF-8（[`Utf16`]），再照 UTF-8 一行一行读（审计 E-3：以前整份读进来再解）。开头认编码读过的
/// `head` 接上剩下的，就是整份；哈希照读过的原字节算。解法同 `String::from_utf16_lossy`：落单的代理换成 `�`，最后多出的
/// 半个字不算。
fn utf16(
    head: Vec<u8>,
    file: File,
    unit: fn([u8; 2]) -> u16,
    (offset, limit): (u64, u64),
    stop: &Stop,
) -> io::Result<Paged> {
    let mut hasher = Hasher::default();
    hasher.update(&head);
    let mut start = Cursor::new(head);
    start.set_position(2);
    let mut reader = BufReader::new(Utf16 {
        bytes: BufReader::new(start.chain(Hashing {
            inner: file,
            hasher,
        })),
        unit,
        ahead: None,
        pending: Vec::new(),
    });
    let page = page(Lines::new(&mut reader, stop), offset, limit)?;
    let (_, rest) = reader.into_inner().bytes.into_inner().into_inner();
    Ok(Paged {
        page,
        hash: Some(rest.hasher.finish()),
    })
}

/// 带 BOM 的 UTF-16 边读边解成 UTF-8 的字节流：两个字节一个码元，照字节序拼；代理对拼成一个字，落单的换成 `�`（同
/// `char::decode_utf16` 换上替换字）；最后只剩半个码元的不算。
struct Utf16<R> {
    bytes: R,
    unit: fn([u8; 2]) -> u16,
    /// 认代理对时多读的那一个码元：不是低代理，下一次从它解起。
    ahead: Option<u16>,
    /// 上一次没交完的那个字剩下的字节。
    pending: Vec<u8>,
}

impl<R: Read> Utf16<R> {
    /// 下一个码元；读完了（最后只剩半个也算）是 `None`。
    fn unit(&mut self) -> io::Result<Option<u16>> {
        if let Some(unit) = self.ahead.take() {
            return Ok(Some(unit));
        }
        let mut pair = [0; 2];
        match self.bytes.read_exact(&mut pair) {
            Ok(()) => Ok(Some((self.unit)(pair))),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// 下一个字；读完了是 `None`。
    fn next_char(&mut self) -> io::Result<Option<char>> {
        let Some(first) = self.unit()? else {
            return Ok(None);
        };
        if !(0xD800..0xDC00).contains(&first) {
            return Ok(Some(
                char::from_u32(u32::from(first)).unwrap_or(char::REPLACEMENT_CHARACTER),
            ));
        }
        match self.unit()? {
            Some(low) if (0xDC00..0xE000).contains(&low) => {
                let code =
                    0x1_0000 + ((u32::from(first) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
                Ok(Some(
                    char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER),
                ))
            }
            next => {
                self.ahead = next;
                Ok(Some(char::REPLACEMENT_CHARACTER))
            }
        }
    }
}

impl<R: Read> Read for Utf16<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut written = 0;
        loop {
            let n = self.pending.len().min(buf.len() - written);
            buf[written..written + n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            written += n;
            if written == buf.len() {
                return Ok(written);
            }
            let Some(found) = self.next_char()? else {
                return Ok(written);
            };
            let mut bytes = [0; 4];
            self.pending
                .extend_from_slice(found.encode_utf8(&mut bytes).as_bytes());
        }
    }
}

/// 照先后给出的每一行，挑出这一页：带行号，一行最长 [`LINE_CHARS`] 个字，一页最多 [`OUTPUT_BYTES`]。每一行都要
/// 读到：一共几行靠它数，整份文件的哈希也靠它读到结尾。不在这一页的只数（[`Source::skip`]）。
fn page(mut lines: impl Source, offset: u64, limit: u64) -> io::Result<Page> {
    let mut text = String::new();
    let mut total = 0;
    let mut to = offset.saturating_sub(1);
    let mut full = false;
    loop {
        // 先看下一行要不要，再读它：不要的不解字。`offset` 不过 `i64::MAX`、`limit` 不过 2000（`read.rs` 的 `Args`），
        // 加起来不会溢出。
        let next = total + 1;
        if next < offset || full || next >= offset + limit {
            if !lines.skip()? {
                break;
            }
            total = next;
            continue;
        }
        let Some(line) = lines.line()? else {
            break;
        };
        total = next;
        let numbered = format!("{total}\t{}\n", cut(&line));
        if text.len() + numbered.len() > OUTPUT_BYTES && total > offset {
            full = true;
            continue;
        }
        text.push_str(&numbered);
        to = total;
    }
    Ok(if total == 0 {
        Page::Empty
    } else if offset > total {
        Page::PastEnd { total }
    } else {
        Page::Lines {
            text,
            from: offset,
            to,
            total,
        }
    })
}

/// 一行最长 [`LINE_CHARS`] 个字，多的截掉、补一个 `…`。
pub(crate) fn cut(line: &str) -> String {
    match line.char_indices().nth(LINE_CHARS) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests;
