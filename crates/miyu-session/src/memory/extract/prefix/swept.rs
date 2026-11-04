//! 接不上时不再整份读（施工 V-43，`docs/construction/V-43-重启后抽取不整份读.md`）：核心重启以后落点记号没了、日志里有撤销恢复
//! 撤回的，原来整份读成事件（大会话 82.8 MB），再整份拼一遍留着一切的有效历史（67 MB）。抽取要的其实只有两样：上次抽到以后的
//! 那一段（拼进请求），和更早的有效历史里认出来的名字、她被叫过的名字、最后的群名（[`Prefix`]）。
//!
//! 所以从头读一遍（整份自检照旧），每一行照先后交给真的有效历史（`miyu_store::sweep`，撤销、恢复、撤回的规矩一行不另写）：
//!
//! - 只解要的种类：上次抽到的那一条以前的只解 [`ONLY`]，以后的再解 [`SAID`]（拼这一段看的她的回复、替她发进群里的话）；人话、
//!   撤销这些有效历史照种类认的本来就全解，她的回复、请求至少解 `seen`；工具结果里派了任务的全解。没解的那几种（工具结果的输出、
//!   注入的事实……）拼这一段、过撤销都用不上，连事件一起不交给有效历史；她的回复只留字和调的工具名。
//! - 有效历史里谁都撤不到了的那一截认完名字就放掉；可能被撤到的那一截以后的留着（撤销的下界照骨架算，同判官、问话列表），
//!   上次抽到以后的一定留着（[`Sweep::holding`]）。
//!
//! 留着的那一截就是整份拼的有效历史（去掉没解的那几种）的后一截，名字接着放掉的那一截认出来的认：渲染、认名字和整份读拼的一个
//! 字节不差。认不准的退回整份读：骨架认不得（同判官）；投影切段（`History::ordered` 照她每条回复的 `seen` 切）在留着的那一截里
//! 的边界不是照先后越来越大的（切段照二分找，少了前面几条回复可能切得不一样），或者那一段跨在留着的那一截两边、两边都有她的回复
//! （留着的那一截里这一段的开头换了一条回复，先后可能不一样）：会话写得出来的日志里都没有（账本查着后一次请求一定看过前一条
//! 回复），外人改过的日志才有。

use std::path::Path;

use miyu_kernel::block::{Block, ToolCall};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_store::log::{Flow, read_marked_with, read_range_with};
use miyu_store::sweep::{Ended, Plan, Settle, Sweep, Tuning};

use super::{Landed, Prefix, Swept};

/// 上次抽到以前要全解的种类：造会话的那一条（子会话自己那一段从哪一条起，渲染人话照它）。人话、撤销这些有效历史照种类认的，
/// 只解要的种类时本来就全解；她的回复、请求只解 `seen`、`error`（投影照它切段，撤销照它认排着的话）。
const ONLY: &[&str] = &["session.created"];

/// 上次抽到以后再解的种类：她的回复（拼这一段的回答、她当场记过没有），别的线替她发进群里的话。渲染人话、过撤销看的别的都在
/// [`ONLY`] 和有效历史照种类认的那几种里；工具结果、请求、注入的事实这些拼这一段用不上（投影只照工具结果排它们自己的先后）。
const SAID: &[&str] = &["session.created", "message.assistant", "venue.delivered"];

/// 工具结果里派出去的任务（有效历史照它记任务：子代理发来的话、派它的那一轮撤掉了的不渲染，抽取的这一段从哪一句起照它）：这一
/// 行里有这几个字的全解。只认字，正文里碰巧有的多解一行，不会少解。
const STARTED: &str = "\"job.started\"";

/// 读一遍会话日志（目录 `dir`），上次抽到第 `after` 条；`seed` 是还接不上、要顺手认出读到的末尾那一处，`ended` 是只取到最后
/// 一条 `turn.ended` 为止（不等闲的那一次）。认不准的交回空的：调的一方整份读。
///
/// # Errors
///
/// 日志读不了：为什么（同整份读；只是不要的种类的正文合 JSON、不合那一种的写法的不报）。
pub(super) fn read(
    dir: &Path,
    after: Seq,
    (seed, ended): (bool, bool),
    tuning: Tuning,
) -> Result<Option<Swept>, String> {
    let hold = after.next();
    let parse = move |line: &str| line_of(line, hold);
    let mut raw = seed.then(Prefix::new);
    let mut last = None;
    let mut first = Pass::new(Plan::guess(), tuning, hold, ended);
    let end = read_marked_with(dir, None, &parse, |events| {
        if let Some(raw) = &mut raw {
            raw.fold(&events);
        }
        last = events.last().map(|event| event.seq).or(last);
        first.feed(events)
    })
    .map_err(|error| error.to_string())?;
    let pass = match first.sweep.ended {
        Ended::Done => first,
        Ended::Unsure => return Ok(None),
        Ended::Again => {
            let reaches = std::mem::take(&mut first.sweep.reaches);
            drop(first);
            let mut again = Pass::new(Plan::exact(reaches), tuning, hold, ended);
            read_range_with(dir, Seq::FIRST, last, &parse, |events| again.feed(events))
                .map_err(|error| error.to_string())?;
            if again.sweep.ended != Ended::Done {
                return Ok(None);
            }
            again
        }
    };
    let from = pass.sweep.from();
    let (history, fold) = pass.sweep.into_tail();
    if !fold.clean(&history, from) {
        return Ok(None);
    }
    Ok(Some(Swept {
        history,
        prefix: fold.prefix,
        seed: end.zip(raw).map(|(mark, prefix)| Landed { mark, prefix }),
    }))
}

/// 解一行：第 `hold` 条以前的只解 [`ONLY`]，`hold` 和以后的只解 [`SAID`]，工具结果里派了任务的全解。读不成的报的错同全解
/// （`Event::from_line_only` 读不成的照全解再读、报它的错）。
fn line_of(line: &str, hold: Seq) -> serde_json::Result<Event> {
    let later = |seq: Seq| seq >= hold;
    let mut event = match seq_written(line) {
        Some(seq) if later(seq) => Event::from_line_only(line, SAID)?,
        Some(_) => Event::from_line_only(line, ONLY)?,
        None => {
            let event = Event::from_line_only(line, ONLY)?;
            match later(event.seq) {
                true => Event::from_line_only(line, SAID)?,
                false => event,
            }
        }
    };
    if matches!(&event.body, Body::Unknown { kind, .. }
        if kind.as_str() == "tool.result" && line.contains(STARTED))
    {
        event = Event::from_line(line)?;
    }
    Ok(event)
}

/// 照写出去的样子认一行的序号（核心写的每一行都以 `{"seq":<数>,` 打头，同 `Event::kind_written`）：不解。对不上的（字段换了
/// 先后、带空白的，核心写不出来）没有，照解出来的认。
fn seq_written(line: &str) -> Option<Seq> {
    let rest = line.strip_prefix("{\"seq\":")?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    rest.get(digits..)?.starts_with(',').then_some(())?;
    rest[..digits].parse().ok().and_then(Seq::new)
}

/// 读的一遍：有效历史，和不等闲的那一次还没见到 `turn.ended` 的那几条（见到了才交，读完了还没见到的不要）。
struct Pass {
    sweep: Sweep<Fold>,
    ended: bool,
    held: Vec<Event>,
}

impl Pass {
    fn new(plan: Plan, tuning: Tuning, hold: Seq, ended: bool) -> Pass {
        Pass {
            sweep: Sweep::new(plan, tuning, Fold::new()).holding(hold),
            ended,
            held: Vec::new(),
        }
    }

    /// 读到的一块，照先后。没解的那几种不交（拼这一段、过撤销都用不上）；她的回复只留字和调的工具名（[`bare`]）。
    fn feed(&mut self, mut events: Vec<Event>) -> Flow {
        events.retain(|event| !matches!(event.body, Body::Unknown { .. }));
        events.iter_mut().for_each(bare);
        if !self.ended {
            return self.sweep.feed(events);
        }
        let Some(k) = events
            .iter()
            .rposition(|event| matches!(event.body, Body::TurnEnded(_)))
        else {
            self.held.append(&mut events);
            return Flow::Go;
        };
        let later = events.split_off(k + 1);
        let mut ready = std::mem::replace(&mut self.held, later);
        match ready.is_empty() {
            true => ready = events,
            false => ready.append(&mut events),
        }
        self.sweep.feed(ready)
    }
}

/// 她的一条回复只留拼这一段看的：字（她的回答），调的工具的名字（当场记过没有）；思考、调用的参数、供应商的数据不留（有效历史只看
/// `seen`）。抽取一直落在后面的会话，留着的那一截从那里起到最后，思考的字原来要占十几兆。
fn bare(event: &mut Event) {
    let Body::MessageAssistant(reply) = &mut event.body else {
        return;
    };
    reply.blocks = std::mem::take(&mut reply.blocks)
        .into_iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(Block::Text(text)),
            Block::ToolCall(call) => Some(Block::ToolCall(ToolCall {
                args: String::new(),
                private: None,
                ..call
            })),
            _ => None,
        })
        .collect();
    // 照原地收集留着原来的容量：放掉多出来的。
    reply.blocks.shrink_to_fit();
}

/// 收下谁都撤不到了的那一截：认名字，记她的回复切段的那几样（[`Fold::clean`] 查）。
struct Fold {
    prefix: Prefix,
    /// 收下的回复里最大的 `seen`：投影切段的边界，都比留着的那一截里的序号小。
    seen: Option<Seq>,
    /// 收下的回复里序号比 `seen` 大的：可能和留着的那一截在投影的同一段里。
    heads: Vec<Seq>,
}

impl Fold {
    fn new() -> Fold {
        Fold {
            prefix: Prefix::new(),
            seen: None,
            heads: Vec::new(),
        }
    }

    /// 留着的那一截（从第 `from` 条起的 `tail`）照投影切段（`History::ordered`：段的边界是每条回复的 `seen`，二分找；一段里
    /// 第一条回复排最前）排出来，和整份拼的那一份里这一截一样。放掉的那一截里的边界都比留着的序号小，二分时对留着的每一条都
    /// 在前面，只差一个常数；所以要的是：留着的那一截里的边界照先后越来越大（相等的也算），跨在两边的那一段不是两边都有她的
    /// 回复（两边都有的，整份拼的照放掉的那一条回复开头，留着的那一截照后面那一条）。只有一边有的一样。
    fn clean(&self, tail: &History, from: Seq) -> bool {
        let replies: Vec<(Seq, Seq)> = tail
            .events()
            .iter()
            .filter_map(|event| match &event.body {
                Body::MessageAssistant(reply) => Some((event.seq, reply.seen)),
                _ => None,
            })
            .collect();
        if replies.windows(2).any(|pair| pair[0].1 > pair[1].1) {
            return false;
        }
        let lower = replies
            .iter()
            .map(|&(_, seen)| seen)
            .filter(|&seen| seen < from)
            .chain(self.seen)
            .max();
        let upper = replies
            .iter()
            .map(|&(_, seen)| seen)
            .find(|&seen| seen >= from);
        let before = self
            .heads
            .iter()
            .any(|&head| lower.is_none_or(|lower| head > lower));
        let after = replies
            .iter()
            .any(|&(seq, _)| upper.is_none_or(|upper| seq <= upper));
        !(before && after)
    }
}

impl Settle for Fold {
    fn settle(&mut self, events: &[Event]) {
        self.prefix.fold(events);
        for event in events {
            if let Body::MessageAssistant(reply) = &event.body {
                self.seen = Some(self.seen.map_or(reply.seen, |seen| seen.max(reply.seen)));
                self.heads.push(event.seq);
            }
        }
        if let Some(seen) = self.seen {
            self.heads.retain(|&head| head > seen);
        }
    }
}
