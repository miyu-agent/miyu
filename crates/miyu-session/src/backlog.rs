//! 订阅时要补发的那一截（施工 3-8 六补，`docs/blueprint/session/actor.md` 第 6 条，`04-核心协议.md` 第七节）：日志里序号
//! 大于 `after`、到订阅那一刻落了盘的最后一条为止的事件。
//!
//! 补到哪一条（`upto`）和订阅在 actor 的同一步里拿：那一步之前落了盘的都推过了，之后的都还没推，所以补的和之后推的
//! 接得上，不重不漏。读日志不在 actor 里：交回一个 [`Backlog`]，拿着它的一方（协议端点）另起阻塞线程读，会话照常跑。
//! `upto` 以前的都落了盘，日志只往后追加，什么时候读都一样；读的时候多出来的不要，它们从订阅推过去。
//!
//! 照序号读（施工 V-37 下）：从第 `after + 1` 条定位着读起，读到 `upto` 那一块就停。原来一段段从头读到最后一段的末尾，
//! `after` 以前的整个解完再扔，`upto` 以后订阅以来新写的也读、也解。

use std::sync::{Mutex, PoisonError, mpsc};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;
use miyu_tool::Log;

use crate::blocking::blocking;

/// 补发的那一截：序号在 `after` 之后、`upto` 为止（含）的事件，还没读。
#[derive(Debug)]
pub struct Backlog {
    log: Log,
    after: u64,
    upto: u64,
}

impl Backlog {
    pub(crate) fn new(log: Log, after: u64, upto: u64) -> Backlog {
        Backlog { log, after, upto }
    }

    /// 补到哪一条：订阅那一刻落了盘的最后一条，也就是订阅推过来的第一条的前一条。它比 `after` 小的，什么都不补。
    pub fn upto(&self) -> u64 {
        self.upto
    }

    /// 在阻塞线程里只读地读出这一截，照先后。一次读完再交回（施工 3-8 六补定：载入会话本来就整份读进内存，不分批）。
    /// 没有要补的（`after` 不比 `upto` 小），不读日志。
    ///
    /// 从第 `after + 1` 条定位着读，读到 `upto` 就停（施工 V-37 下，日志的只读入口的 `read_range`）：`after` 以前的只数
    /// 换行、不解、不自检，`upto` 以后的不读（2026-11-03 项目主人定）。
    ///
    /// # Errors
    ///
    /// 日志读不了、坏了：原因。会话在这时被删了（目录挪走了）也在这里。
    pub async fn read(self) -> Result<Vec<Event>, String> {
        self.gather(|events, kept: &mut Vec<Event>| {
            if kept.is_empty() {
                *kept = events;
            } else {
                kept.extend(events);
            }
        })
        .await
    }

    /// 同 [`Backlog::read`]，只是读出来的每一条当场照 `line` 写成要推出去的那一行，交回写好的行（施工 V-37 下，
    /// `session/actor.md` 第 6 条）：事件写完就放掉，攒着的只有行，原来攒着整份事件、推的时候才一条条写。推出去的字节、
    /// 先后照旧，读不了的照旧在推第一条以前就知道。
    ///
    /// 读出来的一块块分给几个线程写（三个），照块的先后收回：原来写行和头读、解析推过去的同时做，挪到读的时候以后排在
    /// 推第一条以前，一个线程写 23.7 MB 的会话要多等 50 来毫秒。
    ///
    /// # Errors
    ///
    /// 同 [`Backlog::read`]。
    pub async fn lines(
        self,
        line: impl Fn(&Event) -> String + Send + Sync + 'static,
    ) -> Result<Vec<String>, String> {
        let Some((log, from, upto)) = self.range() else {
            return Ok(Vec::new());
        };
        blocking(move || written(&log, (from, upto), &line)).await
    }

    /// 在阻塞线程里读这一截，一块块交给 `take` 收进交回的那一份。没有要补的不读。
    async fn gather<T: Send + 'static>(
        self,
        mut take: impl FnMut(Vec<Event>, &mut Vec<T>) + Send + 'static,
    ) -> Result<Vec<T>, String> {
        let Some((log, from, upto)) = self.range() else {
            return Ok(Vec::new());
        };
        blocking(move || {
            let mut kept = Vec::new();
            log.read_range(from, upto, |events| {
                take(events, &mut kept);
                true
            })?;
            Ok(kept)
        })
        .await
    }

    /// 日志的只读入口、从第几条读到第几条：没有要补的（`after` 不比 `upto` 小）没有。
    fn range(self) -> Option<(Log, Seq, Seq)> {
        let Backlog { log, after, upto } = self;
        if after >= upto {
            return None;
        }
        Some((log, Seq::new(after + 1)?, Seq::new(upto)?))
    }
}

/// 读补发的那一截时几个线程一起写行（施工 V-37 下）。
const WRITERS: usize = 3;

/// 读第 `from` 到第 `upto` 条，每一条照 `line` 写成一行：读出来的一块块排给 [`WRITERS`] 个线程写（最多排两轮），写好的照块的
/// 先后接起来。读不了的交回原因，写好的扔掉。
fn written(
    log: &Log,
    (from, upto): (Seq, Seq),
    line: &(dyn Fn(&Event) -> String + Sync),
) -> Result<Vec<String>, String> {
    let (blocks, inbox) = mpsc::sync_channel::<(usize, Vec<Event>)>(WRITERS * 2);
    let inbox = Mutex::new(inbox);
    let (done, finished) = mpsc::channel::<(usize, Vec<String>)>();
    std::thread::scope(|scope| {
        for _ in 0..WRITERS {
            let done = done.clone();
            let inbox = &inbox;
            scope.spawn(move || {
                loop {
                    let next = inbox.lock().unwrap_or_else(PoisonError::into_inner).recv();
                    let Ok((at, events)) = next else {
                        return;
                    };
                    let lines = events
                        .iter()
                        .map(|event| {
                            // 写的时候留的余量（`format!` 一边写一边翻倍）放掉：整份行攒到推完，23.7 MB 的会话余量占了一倍。
                            // 字不变。
                            let mut one = line(event);
                            one.shrink_to_fit();
                            one
                        })
                        .collect();
                    if done.send((at, lines)).is_err() {
                        return;
                    }
                }
            });
        }
        drop(done);
        let mut count = 0;
        let read = log.read_range(from, upto, |events| {
            let sent = blocks.send((count, events)).is_ok();
            count += 1;
            sent
        });
        drop(blocks);
        let mut slots: Vec<Option<Vec<String>>> = (0..count).map(|_| None).collect();
        for (at, lines) in finished {
            slots[at] = Some(lines);
        }
        read?;
        Ok(slots.into_iter().flatten().flatten().collect())
    })
}

#[cfg(test)]
mod tests;
