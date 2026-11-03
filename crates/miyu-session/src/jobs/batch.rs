//! 监视的命令的标准输出攒成批（施工 MN-1 上，设计 56 第三节第 2 条，`docs/blueprint/session/tools.md`「后台命令」）：一行到
//! 了开一个 [`WINDOW`] 的窗口，窗口里到的行合成一批交出去，记一条 `job.lines`。没有换行结尾的半行等到换行、命令结束，或者
//! 攒满 [`HALF_LINE`] 个字再算一行。
//!
//! 一批边攒边只留头尾（`yes` 这类一个窗口能出几百兆）：最多留截法上限的两倍，多了留头尾各一半，最后照内核的截法拼（头、一行
//! 省了多少个字、尾，`Reports::join`），和内核合并时截出来的一字不差。窗口看时间，留在执行器；她忙着时合成一条在内核里。

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use miyu_kernel::session::Reports;

/// 一行到了开的窗口（设计 56 第三节第 2 条）：窗口里到的行合成一批。
pub(super) const WINDOW: Duration = Duration::from_millis(200);

/// 没有换行结尾的半行最多攒几个字（设计 56 第三节第 2 条）：满了也算一行。
pub(super) const HALF_LINE: usize = 4096;

/// 攒好的一批：截过的字、原来几行、截掉了几个字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Batch {
    /// 这一批的行，行和行之间一个 `\n`，末尾没有；超过 `cut.chars` 的留头尾、中间一行写省了多少。
    pub(crate) text: String,
    /// 原来几行。
    pub(crate) lines: u64,
    /// 截掉了几个字。
    pub(crate) omitted: u64,
}

/// 把 `chunks`（已经合法化过的标准输出，一段一段）读到头，每攒好一批交给 `send`，截法照 `cut`。会等：在自己的线程里调。
/// 读的那一头另起一个线程，这一头才等得了窗口。
pub(super) fn run(
    chunks: Box<dyn Iterator<Item = String> + Send>,
    cut: &Reports,
    mut send: impl FnMut(Batch),
) {
    let (sending, arrived) = mpsc::channel();
    thread::spawn(move || {
        for chunk in chunks {
            if sending.send(chunk).is_err() {
                return;
            }
        }
    });
    let mut partial = String::new();
    let mut gather = Gather::new(cut.chars);
    let mut closes: Option<Instant> = None;
    loop {
        let next = match closes {
            None => arrived.recv().ok(),
            Some(at) => match arrived.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(chunk) => Some(chunk),
                Err(RecvTimeoutError::Timeout) => {
                    send(gather.take(cut));
                    closes = None;
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => None,
            },
        };
        let Some(chunk) = next else {
            break;
        };
        partial.push_str(&chunk);
        while let Some(line) = next_line(&mut partial) {
            gather.push(&line);
        }
        if gather.lines > 0 && closes.is_none() {
            closes = Some(Instant::now() + WINDOW);
        }
    }
    if !partial.is_empty() {
        gather.push(&partial);
    }
    if gather.lines > 0 {
        send(gather.take(cut));
    }
}

/// 从 `partial` 前面切下一行：[`HALF_LINE`] 个字以内有换行的切到换行（不带它）；没有、已经攒满的切前 [`HALF_LINE`] 个字。
/// 都不是的（没换行、还没攒满）留着等下一段。切法和数据怎么分段到的无关。
fn next_line(partial: &mut String) -> Option<String> {
    let mut end = None;
    for (count, (at, c)) in partial.char_indices().enumerate() {
        if c == '\n' {
            let line = partial[..at].to_string();
            partial.drain(..=at);
            return Some(line);
        }
        if count + 1 == HALF_LINE {
            end = Some(at + c.len_utf8());
            break;
        }
    }
    let end = end?;
    Some(partial.drain(..end).collect())
}

/// 攒着的一批：没截过的整份在 `head`；截过的 `head` 是开头那一半，`tail` 留到这时最后的一截。
struct Gather {
    /// 截法的上限。
    limit: usize,
    head: String,
    tail: String,
    /// 一共几个字（行和行之间的 `\n` 算）。
    chars: usize,
    lines: u64,
    cut: bool,
}

impl Gather {
    fn new(limit: usize) -> Gather {
        Gather {
            limit,
            head: String::new(),
            tail: String::new(),
            chars: 0,
            lines: 0,
            cut: false,
        }
    }

    /// 接上一行。超过上限两倍的，头留一半、尾留最后的一半，中间的只记字数。
    fn push(&mut self, line: &str) {
        let piece = match self.lines {
            0 => line.to_string(),
            _ => format!("\n{line}"),
        };
        self.lines += 1;
        self.chars += piece.chars().count();
        match self.cut {
            false => {
                self.head.push_str(&piece);
                if self.chars > self.limit * 2 {
                    let whole = std::mem::take(&mut self.head);
                    self.head = whole.chars().take(self.limit / 2).collect();
                    self.tail = whole.chars().skip(self.limit / 2).collect();
                    self.cut = true;
                    self.trim();
                }
            }
            true => {
                self.tail.push_str(&piece);
                if self.tail.chars().count() > self.limit * 2 {
                    self.trim();
                }
            }
        }
    }

    /// 尾只留最后 `limit - limit / 2` 个字。
    fn trim(&mut self) {
        let keep = self.limit - self.limit / 2;
        let count = self.tail.chars().count();
        if count > keep {
            self.tail = self.tail.chars().skip(count - keep).collect();
        }
    }

    /// 交出这一批，清空了接着攒。
    fn take(&mut self, cut: &Reports) -> Batch {
        let mut taken = std::mem::replace(self, Gather::new(self.limit));
        let lines = taken.lines;
        let wide = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
        if !taken.cut {
            let (text, shortened) = cut.cut(&taken.head);
            let omitted = match shortened {
                true => wide(taken.chars - cut.chars),
                false => 0,
            };
            return Batch {
                text,
                lines,
                omitted,
            };
        }
        taken.trim();
        let omitted = taken.chars - cut.chars;
        Batch {
            text: cut.join(&taken.head, &taken.tail, omitted),
            lines,
            omitted: wide(omitted),
        }
    }
}

#[cfg(test)]
mod tests;
