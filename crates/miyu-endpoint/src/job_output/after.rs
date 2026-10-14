//! `job.output` 带 `after`（施工 V-6 上，`docs/blueprint/protocol.md` 的 `job.output` 第 9、10 条）：头已经有了前 `after`
//! 个字节，只要后面新长出来的那一段，从那里跳过去读，不从头数。带 `wait` 的没有新的就睡着等输出长了、命令结束了再答。

use std::io::{Read, Seek, SeekFrom};

use serde_json::{Value, json};

use miyu_tool::Text;

use super::CAP;

/// 读到的一段。
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Piece {
    /// 交的字：解不开的字节换成 `�`。
    pub(super) output: String,
    /// 到这一段为止一共多少字节：停在一个完整的字后面，下一次照它写 `after`。
    pub(super) size: u64,
    /// 这一段后面还有没交的（一次最多 [`CAP`] 字节）。
    pub(super) more: bool,
}

impl Piece {
    /// 回应的 `result`：格照名字的字母先后排。
    pub(super) fn reply(self, running: bool) -> Value {
        json!({
            "more": self.more,
            "output": self.output,
            "running": running,
            "size": self.size,
        })
    }
}

/// 从 `text` 的第 `after` 个字节读起，最多 [`CAP`] 字节。最后几个字节是半个字的（后面还会写进来、或者被这一次的上限截在了
/// 字中间）先不交，留到下一次：`size` 停在一个完整的字后面。命令已经结束、后面也没有了的照原样交，解不开的换成 `�`。
/// `after` 过了结尾的照结尾算。读到一半读不下去的，读到多少算多少。
pub(super) fn read(mut text: Box<dyn Text>, after: u64, running: bool) -> Piece {
    let Ok(end) = text.seek(SeekFrom::End(0)) else {
        return Piece::default();
    };
    let from = after.min(end);
    if text.seek(SeekFrom::Start(from)).is_err() {
        return Piece {
            size: from,
            ..Piece::default()
        };
    }
    let mut bytes = Vec::new();
    let limit = u64::try_from(CAP).unwrap_or(u64::MAX);
    if text.take(limit).read_to_end(&mut bytes).is_err() && bytes.is_empty() {
        return Piece {
            size: from,
            ..Piece::default()
        };
    }
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let finished = !running && from + read >= end;
    if !finished {
        let half = half_char(&bytes);
        bytes.truncate(bytes.len() - half);
    }
    let size = from + u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    Piece {
        output: String::from_utf8_lossy(&bytes).into_owned(),
        size,
        more: size < end,
    }
}

/// 末尾半个字有几个字节：最后一个字的头一个字节说它该有几个字节，后面没写够的就是半个。不是 UTF-8 的照 0 算（整个交出去，
/// 换成 `�`）。
fn half_char(bytes: &[u8]) -> usize {
    let tail = bytes.len().saturating_sub(4);
    for (at, &byte) in bytes.iter().enumerate().skip(tail).rev() {
        let width = match byte {
            0x00..=0x7F => return 0,
            0x80..=0xBF => continue,
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => return 0,
        };
        let have = bytes.len() - at;
        return if have < width { have } else { 0 };
    }
    0
}

#[cfg(test)]
mod tests;
