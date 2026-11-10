//! 翻页只读要的那一截（施工 V-41 上，`docs/construction/V-41上-翻页不整份读.md`）：原来每一页都把整份日志读成事件、把切点前的
//! 整份喂给投影学派出去的任务，翻到头是 O(N²)。现在三步：
//!
//! 1. 数一遍 `before` 以前的行，不解（`scan_lines`）：照写出去的样子认种类（`Event::kind_written`），记下每一轮
//!    `turn.started` 在哪一行、隔一阵记一个位置；只解派出、了结任务的那几行（`job.reported`、`child.reported`、`tool.nested`，
//!    原文里有 `"job.` 或者转义 `\u` 的 `tool.result`，认不出样子的也解了再看），留下派出、留言叫醒任务的工具结果和那几条。
//! 2. 只读、只解最后几轮：从倒数第 `turns` 个 `turn.started` 读到最后一条（不够 `turns` 轮的从第一条读），照原来的切法切
//!    （`page::cut`：多一轮的不用读，数够了就停在这一截的开头，同整份数到这里）。
//! 3. 切点前要的另外读回那几行：这一页里每一轮的触发消息在这一截前面的、派任务的那次调用所在的回复（投影照它记命令；调用编号
//!    带着回复的序号），从最近记下的位置数过去读那一行。
//!
//! 投影学的是切点前派出、了结任务的那几条加上那几条回复，不是整份：`Projector::learn` 只看回复里的调用、工具结果里派出和留言
//! 叫醒的效果、回报、子会话回报、脚本里调的工具，别的种类、不派任务的工具结果学了也什么都不改，所以学出来的和整份一样。
//! 回复只读派任务那几次调用所在的那一条：调用编号是内核照回复的序号起的，别的回复里不会有同一个编号。
//!
//! 数过去的行不自检（同补发定位跳过的那一截，`store.md` 第 7 条 `read_range`）：读、解了的行照旧报坏了，数过去的那些坏了不报。

use std::collections::BTreeSet;
use std::path::Path;

use memchr::memmem::Finder;

use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::Seq;
use miyu_store::log::{
    BrokenAt, Flow, Line, Mark, OpenError, read_between, read_line_near, scan_lines,
};

use super::page;

/// 一轮的开头。
const TURN: &str = "turn.started";

/// 工具结果：派出、留言叫醒任务的效果在它里面。
const RESULT: &str = "tool.result";

/// 投影学的、整种都留下的：回报、子会话回报、脚本里调的工具。
const KEPT: &[&str] = &["job.reported", "child.reported", "tool.nested"];

/// 至少隔多少行记一个位置：读回触发消息、回复时从最近的一个数过去，最多数这么多行（每一轮的开头、每一段的开头也记）。
const EVERY: u64 = 1024;

/// 读好、切好的一页要的。
pub(crate) struct Slice {
    /// 从倒数第 `turns` 轮的开头（不够的从第一条）读到这一页的最后一条。
    region: Vec<Event>,
    /// 切点在 `region` 里第几条。
    cut: usize,
    /// 因为字节的上限少给了轮数。
    capped: bool,
    /// 切点前还有。
    more: bool,
    /// `region` 前面、这一页要带上的触发消息，照序号。
    outside: Vec<Event>,
    /// 切点前派出、了结任务的那几条和派任务那几次调用所在的回复，照序号：投影照它学，`jobs` 照它找。
    learned: Vec<Event>,
    /// 要读的行里坏了一行，这一页照它以前的读的：坏在哪（施工 YC-6，`view/broken.rs`）。没坏的没有。
    pub(super) broken: Option<BrokenAt>,
}

impl Slice {
    /// 一条都没有的。
    pub(crate) fn empty() -> Slice {
        Slice {
            region: Vec::new(),
            cut: 0,
            capped: false,
            more: false,
            outside: Vec::new(),
            learned: Vec::new(),
            broken: None,
        }
    }

    /// 拼成一页。
    pub(super) fn page(&self) -> page::Page<'_> {
        page::assemble(page::Cut {
            outside: &self.outside,
            region: &self.region,
            cut: self.cut,
            more: self.more,
            capped: self.capped,
            learned: &self.learned,
        })
    }

    /// 投影在这一页以前要学的。
    pub(super) fn learned(&self) -> &[Event] {
        &self.learned
    }
}

/// 读会话目录 `dir` 里序号小于 `before`（没有的是最新一页）的一页：最多 `turns` 轮、`cap` 字节，一条事件多少字节照 `size`
/// 算。`learn` 是要交条目的：投影要学的回复也读回来。
///
/// # Errors
///
/// 没有这个会话；读、解了的行坏了；数得出来的那几样坏了（段末半行、段名接不上）；读写出错。
pub(crate) fn read(
    dir: &Path,
    before: Option<u64>,
    (turns, cap): (usize, usize),
    learn: bool,
    size: &impl Fn(&Event) -> usize,
) -> Result<Slice, OpenError> {
    let seen = scan(dir, before)?;
    let Some((first, end)) = seen.bounds else {
        return Ok(Slice::empty());
    };
    let turns = turns.max(1);
    let start = match seen.starts.len().checked_sub(turns) {
        Some(k) if k > 0 => seen.starts[k],
        _ => first,
    };
    let mut region = Vec::new();
    read_between(dir, &start, &end, |events| {
        if region.is_empty() {
            region = events;
        } else {
            region.extend(events);
        }
        Flow::Go
    })?;
    let (cut, capped) = page::cut(&region, turns, cap, size);
    let more = start != first || cut > 0;
    let cut_at = region
        .get(cut)
        .map_or(end.next.get(), |event| event.seq.get());
    let lines = (first.next.get()..start.next.get(), &seen.marks);
    let mut outside = Vec::new();
    for &seq in page::triggers(&region[cut..]).range(lines.0.clone()) {
        outside.extend(near(dir, &lines, seq)?);
    }
    let mut learned: Vec<Event> = seen
        .kept
        .into_iter()
        .filter(|event| event.seq.get() < cut_at)
        .collect();
    if learn {
        let have: BTreeSet<u64> = learned.iter().map(|event| event.seq.get()).collect();
        let calls: BTreeSet<u64> = learned
            .iter()
            .filter_map(|event| match &event.body {
                Body::ToolResult(result)
                    if result
                        .effects
                        .iter()
                        .any(|effect| matches!(effect, Effect::JobStarted(_))) =>
                {
                    Some(result.call_id.message().get())
                }
                _ => None,
            })
            .filter(|seq| !have.contains(seq) && first.next.get() <= *seq && *seq < cut_at)
            .collect();
        let lines = (first.next.get()..cut_at, &seen.marks);
        for seq in calls {
            learned.extend(near(dir, &lines, seq)?);
        }
        learned.sort_by_key(|event| event.seq);
    }
    Ok(Slice {
        region,
        cut,
        capped,
        more,
        outside,
        learned,
        broken: None,
    })
}

/// 读回第 `seq` 条：从 `marks` 里在它前面最近的那一个位置数过去。不在 `range` 里的、找不到的没有。
fn near(
    dir: &Path,
    (range, marks): &(std::ops::Range<u64>, &Vec<Mark>),
    seq: u64,
) -> Result<Option<Event>, OpenError> {
    let (true, Some(seq)) = (range.contains(&seq), Seq::new(seq)) else {
        return Ok(None);
    };
    let k = marks.partition_point(|mark| mark.next <= seq);
    match k.checked_sub(1) {
        Some(k) => read_line_near(dir, &marks[k], seq),
        None => Ok(None),
    }
}

/// 数一遍记下的。
#[derive(Default)]
struct Seen {
    /// 第一行的开头、最后一行后面那一行的开头。
    bounds: Option<(Mark, Mark)>,
    /// 每一轮 `turn.started` 那一行的开头，照先后。
    starts: Vec<Mark>,
    /// 记下的位置，照先后：每一段的第一行、每一轮的开头，此外至少每 [`EVERY`] 行一个。
    marks: Vec<Mark>,
    /// 派出、了结任务的那几条，照先后。
    kept: Vec<Event>,
}

/// 照写出去的样子认不了、要找的字。
struct Needles {
    /// 工具结果里派出、留言叫醒任务的效果：`"kind":"job.started"`、`"kind":"job.messaged"`。
    job: Finder<'static>,
    /// 转义：写成 `\u` 的字（内核只这样写控制字符）凑得出别的写法，有的照解。
    escaped: Finder<'static>,
}

/// 数 `dir` 里序号小于 `before` 的行，记下要的。
fn scan(dir: &Path, before: Option<u64>) -> Result<Seen, OpenError> {
    let needles = Needles {
        job: Finder::new(b"\"job."),
        escaped: Finder::new(b"\\u"),
    };
    let mut seen = Seen::default();
    let mut failed = None;
    scan_lines(
        dir,
        before,
        &mut |line| match look(&line, &needles, &mut seen) {
            Ok(()) => Flow::Go,
            Err(error) => {
                failed = Some(error);
                Flow::Stop
            }
        },
    )?;
    match failed {
        Some(error) => Err(error),
        None => Ok(seen),
    }
}

/// 看一行：记位置，要的解了留下。
fn look(line: &Line<'_>, needles: &Needles, seen: &mut Seen) -> Result<(), OpenError> {
    let start = line.start;
    seen.bounds = Some((seen.bounds.map_or(start, |(first, _)| first), line.end()));
    let mut turn = false;
    match Event::kind_written(line.text) {
        Some(kind) if kind == TURN => turn = true,
        Some(kind) if KEPT.contains(&kind) => seen.kept.push(line.event()?),
        Some(kind) if kind == RESULT => {
            if needles.job.find(line.text).is_some() || needles.escaped.find(line.text).is_some() {
                keep(line.event()?, &mut seen.kept);
            }
        }
        Some(_) => {}
        None => {
            let event = line.event()?;
            turn = matches!(event.body, Body::TurnStarted(_));
            keep(event, &mut seen.kept);
        }
    }
    if turn {
        seen.starts.push(start);
    }
    let far = seen
        .marks
        .last()
        .is_none_or(|mark| line.seq.get() - mark.next.get() >= EVERY);
    if turn || far || start.bytes == 0 {
        seen.marks.push(start);
    }
    Ok(())
}

/// 投影学、`jobs` 找的那几种留下：派出、留言叫醒任务的工具结果，回报、子会话回报、脚本里调的工具。
fn keep(event: Event, kept: &mut Vec<Event>) {
    let wanted = match &event.body {
        Body::ToolResult(result) => result
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::JobStarted(_) | Effect::JobMessaged(_))),
        Body::JobReported(_) | Body::ChildReported(_) | Body::ToolNested(_) => true,
        _ => false,
    };
    if wanted {
        kept.push(event);
    }
}

#[cfg(test)]
pub(super) mod tests;
