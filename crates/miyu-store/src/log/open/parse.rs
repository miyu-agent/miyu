//! 一段日志的每一行读成事件（施工 V-2 下补）：多的分几块在几个线程里一起读，读完照先后交回。一万条事件的会话读、解日志
//! 约 23 ms，是重启以后打开大会话的大头；读成事件和别的行不相干，序号接没接上留给调的一方照先后查。
//!
//! 分块照字节分（施工 V-12）：每块从一行开头起、到一行末尾止，切行也在各自的线程里（`memchr` 找换行）。原来先在一个线程里
//! 把整段切成一行行再分，一万条的会话切行就占 4 到 18 ms。
//!
//! 交回的和一行行读的一样：照先后每一行一项，读不成的那一行是第几行（从 1 数）、为什么；第一处读不成的后面的不交（一块里读不
//! 成就不往下读了）。

use miyu_kernel::event::Event;

/// 字节到了这么多才分块：少的一个线程读完比起线程还快。
const SPLIT_AT: usize = 1024 * 1024;

/// 最多分几块。
const MOST: usize = 8;

/// 读成的一行是事件；读不成的是第几行和为什么。
pub(super) type Parsed = Result<Event, (usize, String)>;

/// `bytes` 是整行（每行以 `\n` 结尾，最后一行也是），每一行读成事件，照先后交回：第 `k` 项是第 `k + 1` 行。
pub(super) fn lines(bytes: &[u8]) -> Vec<Parsed> {
    let cuts = cuts(bytes, parts(bytes.len()));
    if cuts.len() <= 2 {
        return chunk(bytes);
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = cuts
            .windows(2)
            .map(|pair| {
                let part = &bytes[pair[0]..pair[1]];
                scope.spawn(move || chunk(part))
            })
            .collect();
        let mut all: Vec<Parsed> = Vec::new();
        for handle in handles {
            // 读一行不会 panic；真的 panic 了，照原样抛出去，和一个线程读的时候一样。
            let part = handle
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            // 块里的行号从 1 数：接上前面几块读成的行数。前面的块都读完了（读不成的那一块之后的不接），行数是准的。
            let before = all.len();
            let stop = part.last().is_some_and(Result::is_err);
            all.extend(
                part.into_iter()
                    .map(|one| one.map_err(|(number, why)| (before + number, why))),
            );
            if stop {
                break;
            }
        }
        all
    })
}

/// 分几块：少的不分；多的照机器的核数，最多 [`MOST`]。
fn parts(bytes: usize) -> usize {
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

/// 一块整行：第一行是第 1 行。读不成的那一行交了就停。
fn chunk(part: &[u8]) -> Vec<Parsed> {
    let mut parsed = Vec::new();
    let mut start = 0;
    for end in memchr::memchr_iter(b'\n', part) {
        let line = &part[start..end];
        start = end + 1;
        let number = parsed.len() + 1;
        let one = std::str::from_utf8(line)
            .map_err(|_| (number, "not UTF-8".to_string()))
            .and_then(|text| {
                Event::from_line(text).map_err(|error| (number, format!("not readable: {error}")))
            });
        let stop = one.is_err();
        parsed.push(one);
        if stop {
            break;
        }
    }
    parsed
}

#[cfg(test)]
mod tests;
