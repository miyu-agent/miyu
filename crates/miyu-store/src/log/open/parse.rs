//! 一段日志的每一行读成事件（施工 V-2 下补）：多的分几块在几个线程里一起读，解好一块照先后交一块（施工 V-15，`stream.rs`）。一万条事件的会话读、解日志
//! 约 23 ms，是重启以后打开大会话的大头；读成事件和别的行不相干，序号接没接上留给调的一方照先后查。
//!
//! 分块照字节分（施工 V-12）：每块从一行开头起、到一行末尾止，切行也在各自的线程里（`memchr` 找换行）。原来先在一个线程里
//! 把整段切成一行行再分，一万条的会话切行就占 4 到 18 ms。
//!
//! 交回的和一行行读的一样：读成的事件照先后，读不成的那一行是第几行（从 1 数）、为什么；第一处读不成的后面的不交（一块里读不
//! 成就不往下读了）。

use miyu_kernel::event::Event;

mod stream;

pub use stream::Flow;
pub(super) use stream::stream;

/// 字节到了这么多才分块：少的一个线程读完比起线程还快。
const SPLIT_AT: usize = 1024 * 1024;

/// 最多几个线程一起解。
const MOST: usize = 8;

/// 一块块交的时候（[`stream()`]），每个线程平均分到几块：块小了，交出去的一方等第一块等得短，留在内存里的也少。
const PER_THREAD: usize = 8;

/// 一块（或者整段）读成的：读成的事件照先后；读不成的那一行是第几行（从 1 数）、为什么，它后面的不读。
pub(super) struct Lines {
    /// 读成的事件，第 `k` 项是第 `k + 1` 行。
    pub(super) events: Vec<Event>,
    /// 读不成的那一行：在 `events` 后面紧跟着的那一行。
    pub(super) broken: Option<(usize, String)>,
}

/// 几个线程一起解：少的一个；多的照机器的核数，最多 [`MOST`]。
fn threads(bytes: usize) -> usize {
    if bytes < SPLIT_AT {
        return 1;
    }
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    cores.min(MOST).min(bytes / (SPLIT_AT / 2)).max(1)
}

/// 照 `parts` 块大致等分，每一刀挪到一行开头：交回各块的起点和最后的终点（`0`、…、`bytes.len()`）。一行特别长的，挨着的
/// 两刀可能落在同一处，并成一块。
fn cuts(bytes: &[u8], parts: usize) -> Vec<usize> {
    let mut cuts = vec![0];
    for k in 1..parts {
        let at = (bytes.len() * k / parts).max(cuts[cuts.len() - 1]);
        let Some(newline) = memchr::memchr(b'\n', &bytes[at..]) else {
            break;
        };
        let cut = at + newline + 1;
        if cut < bytes.len() && cut > cuts[cuts.len() - 1] {
            cuts.push(cut);
        }
    }
    cuts.push(bytes.len());
    cuts
}

/// 一块整行：第一行是第 1 行。读不成的那一行记下就停。
fn chunk(part: &[u8]) -> Lines {
    let mut events = Vec::with_capacity(memchr::memchr_iter(b'\n', part).count());
    let mut start = 0;
    for end in memchr::memchr_iter(b'\n', part) {
        let line = &part[start..end];
        start = end + 1;
        let number = events.len() + 1;
        let one = std::str::from_utf8(line)
            .map_err(|_| (number, "not UTF-8".to_string()))
            .and_then(|text| {
                Event::from_line(text).map_err(|error| (number, format!("not readable: {error}")))
            });
        match one {
            Ok(event) => events.push(event),
            Err(broken) => {
                return Lines {
                    events,
                    broken: Some(broken),
                };
            }
        }
    }
    Lines {
        events,
        broken: None,
    }
}

#[cfg(test)]
mod tests;
