//! 怎么切一页（施工 9-6 下，`docs/blueprint/protocol.md`「`view.page`」）：纯函数，读日志、写回应在 `view.rs`。
//!
//! - 页的边界落在回合之间：从 `before` 往前数 `turn.started`，每一轮从它的 `turn.started` 起，到下一轮的 `turn.started` 之前；
//!   不在回合里的事件跟着它们所在的位置走。数到第一轮的，连前面的（`session.created` 这些）一起给，`more` 是假。
//! - 数够 `turns` 轮，或者再加一轮就超过 `cap` 字节，停；至少给一整轮，一轮自己超了也整轮给。因为字节停的记 `capped`。
//! - 切点前的触发消息（这一页里每一轮 `turn.started` 的 `trigger`）也带上，排在最前面：可能和更早一页重复，头照序号去重。
//!   `first` 照切点算，不算带进来的触发消息：往前翻拿它当 `before`。
//! - 这一页里报完了的任务（`job.reported`、`child.reported`），派它的 `job.started` 在切点前的，带上它派出时的样子（施工 9-6
//!   再补）：头照它写「后台命令 xxx 跑完了」那一行。

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use miyu_kernel::event::{Body, Effect, Event, JobStarted};
use miyu_kernel::id::JobId;

/// 切出来的一页。
#[derive(Debug, PartialEq)]
pub(super) struct Page<'a> {
    /// 这一页的事件，照序号：先是带进来的触发消息，再是切点到 `before` 之前的。
    pub(super) events: Vec<&'a Event>,
    /// 切点的序号：往前翻拿它当 `before`。一条都没有的没有。
    pub(super) first: Option<u64>,
    /// 这一页最后一条的序号。一条都没有的没有。
    pub(super) last: Option<u64>,
    /// 切点前还有更早的。
    pub(super) more: bool,
    /// 因为字节的上限少给了轮数。
    pub(super) capped: bool,
    /// 这一页里报完了、在切点前派出去的任务，照派出的先后。
    pub(super) jobs: Vec<&'a JobStarted>,
}

/// 从 `events`（整份日志，照序号）里切 `before` 之前的一页（没有的是最新一页）：最多 `turns` 轮、`cap` 字节，一条事件多少
/// 字节照 `size` 算。整份在内存里的切法，同 [`cut`]、[`assemble`] 拼起来的；读日志的那一边只读要的那一截（`read.rs`）。
#[cfg(test)]
pub(super) fn page<'a>(
    events: &'a [Event],
    before: Option<u64>,
    turns: usize,
    cap: usize,
    size: impl Fn(&Event) -> usize,
) -> Page<'a> {
    let end = before.map_or(events.len(), |before| {
        events.partition_point(|event| event.seq.get() < before)
    });
    let (cut, capped) = cut(&events[..end], turns, cap, &size);
    assemble(Cut {
        outside: &[],
        region: &events[..end],
        cut,
        more: cut > 0,
        capped,
        learned: &events[..cut],
    })
}

/// 切点：`region`（从一轮的开头或者日志的第一条起，到这一页的最后一条）里从哪一条起给，和是不是因为字节停的。`region` 从一轮
/// 的开头起的，里面要正好有 `turns` 轮（多一轮的不用读）：数够了就在它的开头停，同整份数到这里（`read.rs`）。
pub(super) fn cut(
    region: &[Event],
    turns: usize,
    cap: usize,
    size: &impl Fn(&Event) -> usize,
) -> (usize, bool) {
    let starts: Vec<usize> = (0..region.len())
        .filter(|&index| matches!(region[index].body, Body::TurnStarted(_)))
        .collect();
    cut_at(region, &starts, turns.max(1), cap, size)
}

/// 这一页（`page`，切点到最后一条）里每一轮的触发消息的序号。
pub(super) fn triggers(page: &[Event]) -> BTreeSet<u64> {
    page.iter()
        .filter_map(|event| match &event.body {
            Body::TurnStarted(started) => started.trigger.map(|seq| seq.get()),
            _ => None,
        })
        .collect()
}

/// 拼一页要的（[`assemble`]）。
pub(super) struct Cut<'a> {
    /// `region` 前面读回来的触发消息，照序号：都比 `region` 的第一条早。
    pub(super) outside: &'a [Event],
    /// 读到的那一截：切点在它里面。
    pub(super) region: &'a [Event],
    /// 切点在 `region` 里第几条。
    pub(super) cut: usize,
    /// 切点前还有。
    pub(super) more: bool,
    /// 因为字节的上限少给了轮数。
    pub(super) capped: bool,
    /// 切点前的日志里，派出、了结任务的那几条（多了别的也行），照序号：`jobs` 照它找。
    pub(super) learned: &'a [Event],
}

/// 照切点拼一页：切点前的触发消息（`outside` 里的、`region` 切点前的）排在最前面，再是切点以后的；`jobs` 照 `learned` 找。
pub(super) fn assemble(cut: Cut<'_>) -> Page<'_> {
    let Cut {
        outside,
        region,
        cut,
        more,
        capped,
        learned,
    } = cut;
    let triggers = triggers(&region[cut..]);
    let earlier = outside
        .iter()
        .chain(&region[..cut])
        .filter(|event| triggers.contains(&event.seq.get()));
    let page: Vec<&Event> = earlier.chain(&region[cut..]).collect();
    Page {
        jobs: started_before(learned, &page),
        events: page,
        first: region.get(cut).map(|event| event.seq.get()),
        last: region.last().map(|event| event.seq.get()),
        more,
        capped,
    }
}

/// `page` 里报完了的任务，派它的 `job.started` 在 `before` 里的，照派出的先后（也就是编号）。回报常是下一轮的触发消息，在
/// 切点前、照样算。
fn started_before<'a>(before: &'a [Event], page: &[&Event]) -> Vec<&'a JobStarted> {
    let reported: BTreeSet<&JobId> = page
        .iter()
        .filter_map(|event| match &event.body {
            Body::JobReported(reported) => Some(&reported.job),
            Body::ChildReported(reported) => Some(&reported.job),
            _ => None,
        })
        .collect();
    before
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.effects.iter()),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            Effect::JobStarted(started) if reported.contains(&started.job) => Some(started),
            _ => None,
        })
        .collect()
}

/// 切点：`events` 里从哪一条起给；和是不是因为字节停的。`starts` 是每一轮 `turn.started` 的位置。
fn cut_at(
    events: &[Event],
    starts: &[usize],
    turns: usize,
    cap: usize,
    size: &impl Fn(&Event) -> usize,
) -> (usize, bool) {
    let Some(&newest) = starts.last() else {
        return (0, false);
    };
    let bytes = |from: usize, to: usize| events[from..to].iter().map(size).sum::<usize>();
    let mut cut = newest;
    let mut used = bytes(newest, events.len());
    // `taken`：已经给了几轮；每一轮从更早的一个 `turn.started` 起。
    for (taken, &start) in (1..).zip(starts.iter().rev().skip(1)) {
        if taken == turns {
            return (cut, false);
        }
        let more = bytes(start, cut);
        if used + more > cap {
            return (cut, true);
        }
        used += more;
        cut = start;
    }
    // 数到了第一轮：它前面的（`session.created` 这些）一起给；加上它就超了字节的，留到下一页。
    if cut > 0 && used + bytes(0, cut) > cap {
        return (cut, true);
    }
    (0, false)
}
