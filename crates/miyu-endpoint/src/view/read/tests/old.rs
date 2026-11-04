//! 以前的读法（施工 V-41 上的对照测试用，照抄 `origin/main` 的 `view.rs`、`view/page.rs`，只改了这几处）：整份读进内存
//! （`read_events`），从整份里切一页、把切点前的整份交给投影学。改的：字节的上限、字从参数来（以前是常量 `CAP`、每次读
//! 资源目录），`Page` 在这里另写一份，`learn` 照旧交整份。

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Effect, Event, JobStarted};
use miyu_kernel::id::JobId;
use miyu_store::blob::Blobs;
use miyu_view::{Projector, Texts};

use super::super::super::project;

/// 切出来的一页（照抄 `page.rs` 的 `Page`）。
pub(super) struct Page<'a> {
    pub(super) events: Vec<&'a Event>,
    pub(super) first: Option<u64>,
    pub(super) last: Option<u64>,
    pub(super) more: bool,
    pub(super) capped: bool,
    pub(super) jobs: Vec<&'a JobStarted>,
}

/// 照抄 `view.rs` 的 `paged`：`cap` 以前是常量。
pub(super) fn paged(
    events: &[Event],
    before: Option<u64>,
    (turns, cap): (usize, usize),
    view: Option<(Arc<Texts>, Blobs)>,
) -> (Value, Option<Projector>) {
    let cut = page(events, before, turns, cap, |event| {
        serde_json::to_string(event).map_or(0, |line| line.len())
    });
    let (mut reply, projector) = match view {
        Some((texts, blobs)) => {
            // 切点前的日志只学派出去的任务。
            let earlier = cut.first.map_or(events.len(), |first| {
                events.partition_point(|event| event.seq.get() < first)
            });
            let projector = project::fed(texts, blobs, &events[..earlier], &cut.events);
            (
                json!({"entries": projector.entries(), "more": cut.more}),
                Some(projector),
            )
        }
        None => (json!({"events": cut.events, "more": cut.more}), None),
    };
    if let (Some(first), Some(last)) = (cut.first, cut.last) {
        reply["first"] = json!(first);
        reply["last"] = json!(last);
    }
    if cut.capped {
        reply["capped"] = json!(true);
    }
    // 这一页里报完了、在切点前派出去的任务（施工 9-6 再补）：没有的不写。
    if !cut.jobs.is_empty() {
        reply["jobs"] = json!(cut.jobs);
    }
    (reply, projector)
}

// 以下照抄 `page.rs`。

/// 从 `events`（整份日志，照序号）里切 `before` 之前的一页（没有的是最新一页）：最多 `turns` 轮、`cap` 字节，一条事件多少
/// 字节照 `size` 算。
fn page<'a>(
    events: &'a [Event],
    before: Option<u64>,
    turns: usize,
    cap: usize,
    size: impl Fn(&Event) -> usize,
) -> Page<'a> {
    let end = before.map_or(events.len(), |before| {
        events.partition_point(|event| event.seq.get() < before)
    });
    let starts: Vec<usize> = (0..end)
        .filter(|&index| matches!(events[index].body, Body::TurnStarted(_)))
        .collect();
    let (cut, capped) = cut_at(&events[..end], &starts, turns.max(1), cap, &size);
    let triggers: BTreeSet<u64> = events[cut..end]
        .iter()
        .filter_map(|event| match &event.body {
            Body::TurnStarted(started) => started.trigger.map(|seq| seq.get()),
            _ => None,
        })
        .collect();
    let first = events[..end].get(cut).map(|event| event.seq.get());
    let earlier = events[..cut]
        .iter()
        .filter(|event| triggers.contains(&event.seq.get()));
    let page: Vec<&Event> = earlier.chain(&events[cut..end]).collect();
    Page {
        jobs: started_before(&events[..cut], &page),
        events: page,
        first,
        last: end.checked_sub(1).map(|index| events[index].seq.get()),
        more: cut > 0,
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
