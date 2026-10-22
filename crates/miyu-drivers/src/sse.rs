//! SSE 分帧（WHATWG HTML 标准「Server-sent events」里的解析规矩）：字节一片一片进，一条一条的
//! 事件出。分片从哪里切开都一样。
//!
//! - 换行：`\n`、`\r\n`、`\r` 都算；`\r\n` 被切在两片之间，不多出一个空行。
//! - 一行收全了才按 UTF-8 解：一个汉字被切在两片之间也不坏。坏的字节换成 U+FFFD，规矩就是这样。
//! - `data:` 后面的一个空格去掉，几行 `data` 用换行接起来；`event:` 记下名字；`:` 开头的是注释；
//!   `id:`、`retry:` 和不认识的字段不理。空行结束一条，有 `data` 的才交出来。
//! - 流断在半条上：[`Sse::finish`] 把最后一条照样交出来。
//! - 一条事件连同还没收全的那一行攒过上限（64 MiB）：不再往下收，[`Sse::overflowed`] 为真，解码器照可重试的出错收尾（鲁棒性 B-3）。

use std::mem;

use miyu_kernel::event::{CallError, ErrorClass};

/// 一条事件（连同还没收全的那一行）最多攒多少字节（鲁棒性 B-3）：供应商、中转站一直发、不换行或者一直不空行，读流那一头
/// 只有空闲超时，不停地发就永远不超时，缓冲不能一直涨到内存用完。64 MiB：一条事件里塞一整张 base64 的大图也装得下。
pub(crate) const MOST: usize = 64 * 1024 * 1024;

/// 一条事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// `event:` 写的名字；没写的是 `None`（规矩里的默认名字是 `message`）。
    pub event: Option<String>,
    /// `data`，几行用换行接起来。
    pub data: String,
}

/// 分帧的状态：还没收全的一行，和这一条攒到的字段。
#[derive(Debug, Clone, Default)]
pub struct Sse {
    /// 还没收全的一行。
    line: Vec<u8>,
    /// 上一个字节是 `\r`：紧跟着的 `\n` 是同一个换行。
    after_cr: bool,
    /// 这一条的 `data`，每一行后面都跟一个换行，交出来时去掉最后那个。
    data: String,
    /// 这一条的名字。
    event: Option<String>,
    /// 攒过了 [`MOST`]：清掉了攒的，再来的都不收。
    overflowed: bool,
}

impl Sse {
    /// 空的。
    pub fn new() -> Sse {
        Sse::default()
    }

    /// 喂一片字节，交回这一片里收全了的事件。
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Event> {
        let mut out = Vec::new();
        for &byte in bytes {
            if self.overflowed {
                break;
            }
            let after_cr = mem::replace(&mut self.after_cr, byte == b'\r');
            match byte {
                b'\n' if after_cr => {}
                b'\n' | b'\r' => self.end_line(&mut out),
                _ => self.line.push(byte),
            }
            if self.line.len() + self.data.len() > MOST {
                *self = Sse {
                    overflowed: true,
                    ..Sse::default()
                };
            }
        }
        out
    }

    /// 攒过了上限（64 MiB）：这一条不收了，再喂的也不收。
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// 流完了：没收全的最后一行、最后一条，照样交出来。
    pub fn finish(mut self) -> Vec<Event> {
        let mut out = Vec::new();
        if !self.line.is_empty() {
            self.end_line(&mut out);
        }
        self.dispatch(&mut out);
        out
    }

    /// 收全了一行。
    fn end_line(&mut self, out: &mut Vec<Event>) {
        let line = String::from_utf8_lossy(&mem::take(&mut self.line)).into_owned();
        if line.is_empty() {
            self.dispatch(out);
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_str(), ""),
        };
        match field {
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
            }
            "event" => self.event = Some(value.to_string()),
            _ => {}
        }
    }

    /// 空行：有 `data` 的交出一条，名字和 `data` 都清掉。
    fn dispatch(&mut self, out: &mut Vec<Event>) {
        let event = self.event.take();
        if self.data.is_empty() {
            return;
        }
        let mut data = mem::take(&mut self.data);
        data.pop();
        out.push(Event { event, data });
    }
}

/// 攒过了 [`MOST`] 的出错：可重试，断开这一次（鲁棒性 B-3）。三家的解码器喂完一片看 [`Sse::overflowed`]，照它收尾。
pub(crate) fn overflow_error() -> CallError {
    CallError {
        class: ErrorClass::Retryable,
        message: format!("流里一条事件超过 {} MiB，不再往下读", MOST >> 20),
        status: None,
        retry_after_ms: None,
    }
}

#[cfg(test)]
mod tests;
