//! 带工具调用的那一批晚一点同步（施工 BO-4，`internal/blueprint/session/actor.md` 第 5 条第 9 款）：等这一回复要的工具都出了
//! 结果、结果写进去了再同步，一次盖住两批；工具跑得久的，最多等 [`AFTER_TOOLS`]。
//!
//! 为什么：同步是对会话日志做 `sync_data`，写工具的结果是往同一个文件后面追加。btrfs 上同步的那一阵拿着这个文件的锁，这时
//! 候的追加要等它做完。原来带工具调用的回复一写完就派同步，读文件、找文件这类不到 1 毫秒就出结果的工具，写结果的那一下
//! 正撞在同步上；盘闲了一会儿以后头一次同步又特别慢。2026-11-27 在另一台测试机上量：每步之前隔 0.3 秒，读文件的一步从
//! 连着跑的 1.5 毫秒变成 10.8 毫秒，核心的日志里多出来的 8 毫秒全在「工具跑完」到「发下一个请求」之间；单独探盘，闲 0.3 秒
//! 以后同步一次 10.3 毫秒、这期间往同一个文件追加被挡 6.9 毫秒，闲 2.5 秒以后是 27 毫秒、23 毫秒。真模型答一步都不止
//! 0.3 秒，所以实际使用时每一次读文件都这样等。
//!
//! 晚一点同步丢不丢东西：同步本来就在后台做（施工 V-19），工具不等它；这里只是把开始的时刻从「回复写完」挪到「结果写完」，
//! 最晚不超过 [`AFTER_TOOLS`]。断电以后日志里看不到这一次派工具的那一小段时间，原来是一次同步那么长，现在最多多这 100 毫秒；
//! 工具出结果快的，结果也一起同步了。有回应等着同步的（人发来的话）、有别的非同步不可的一批的，马上做，不等。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;

/// 带工具调用的那一批最多晚多久同步。读文件、找文件、改文件、大多数短命令都在这以内出结果；更久的照旧在工具跑着的时候同步。
pub(crate) const AFTER_TOOLS: Duration = Duration::from_millis(100);

/// 一批里和这件事有关的：带了几个工具调用、出了几个结果、能不能晚一点同步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Seen {
    calls: usize,
    results: usize,
    /// 这一批要同步的理由只有「带工具调用的回复」：外面交来的、一轮收尾的不在里面。
    patient: bool,
}

impl Seen {
    /// 看一批。
    pub(crate) fn of(events: &[Event]) -> Seen {
        let calls = events
            .iter()
            .filter_map(|event| match &event.body {
                Body::MessageAssistant(reply) => Some(
                    reply
                        .blocks
                        .iter()
                        .filter(|block| matches!(block, Block::ToolCall(_)))
                        .count(),
                ),
                _ => None,
            })
            .sum();
        let results = events
            .iter()
            .filter(|event| matches!(event.body, Body::ToolResult(_)))
            .count();
        let patient = events.iter().all(|event| {
            matches!(event.by, By::Model(_) | By::Tool(_) | By::Kernel)
                && !matches!(
                    event.body,
                    Body::SessionCreated(_)
                        | Body::TurnEnded(_)
                        | Body::TurnReverted(_)
                        | Body::TurnUnreverted(_)
                )
        });
        Seen {
            calls,
            results,
            patient,
        }
    }
}

/// 晚一点同步的账：最近那一条带工具调用的回复还有几个调用没出结果；正等着的那一次同步怎么叫它现在就做。
#[derive(Debug, Default)]
pub(crate) struct Later {
    out: usize,
    waiting: Option<Arc<Notify>>,
}

impl Later {
    /// 写进去了一批：`must` 是这一批要同步，`starting` 是这一批当场派了一次同步。交回的不是空的，这一次同步先等它（最多
    /// [`AFTER_TOOLS`]）再做：这一批只因为带了工具调用才要同步，调用还没出结果。别的情况不等；这一批非同步不可、或者调用
    /// 都出了结果的，把正等着的那一次叫起来。
    pub(crate) fn written(
        &mut self,
        seen: Seen,
        must: bool,
        starting: bool,
    ) -> Option<Arc<Notify>> {
        // 只数最近那一条回复的：更早的回复没等到结果的（被打断的）不再等。
        if seen.calls > 0 {
            self.out = seen.calls;
        }
        self.out = self.out.saturating_sub(seen.results);
        if starting && seen.patient && self.out > 0 {
            let go = Arc::new(Notify::new());
            self.waiting = Some(Arc::clone(&go));
            return Some(go);
        }
        if must || self.out == 0 {
            self.go();
        }
        None
    }

    /// 不等了，现在就做：有回应等着同步、要停下了、非同步不可的一批来了、工具都出了结果。没有在等的什么都不做。
    pub(crate) fn go(&mut self) {
        if let Some(go) = self.waiting.take() {
            go.notify_one();
        }
    }
}

#[cfg(test)]
mod tests;
