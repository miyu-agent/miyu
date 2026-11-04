//! 边读边挑（施工 V-45）：日志一块块读，照先后交给真的有效历史（`miyu_store::sweep`，同判官、问话列表），前面谁都撤不到的
//! 那一截定下来就挑（[`Collect`]）、放掉。不留整份的有效历史，也不留整份的条目：「读」只留这一页可能放得下的头几条，「找」只留
//! 这一页可能放得下的最新几条和摘出来的一段，别的数一数就扔。
//!
//! 挑出来的和整份读、整份算（施工 6-4 的读法，`history/tests/same/old.rs` 照抄着当准绳）逐字节一样，靠这几样：
//!
//! - 定下来的一截截照序号接着交，还算数的事件照序号排着（有效历史的规矩），人切权限级别的（照读来的原样挑）照序号插进去；
//! - 她自己翻记录的调用编号照定下来的回复记：调用编号带着回复的序号（`kernel/ids.md`），回复总在它的结果前面定下来。对不上这条
//!   规矩的日志（账本过不了的：回复里的编号不是它自己的序号、结果的编号不在它前面）退回整份读；
//! - 别的会话发来的话，「谁」那一格要照整份日志记着的派出去的任务认（`History::is_peer`）：读到的每一条都记一遍
//!   （[`Kept::note`]，只记任务、待办、包的状态，不留事件），这一页的几条读完再认；
//! - 一页放不放得下照不算「谁」那一格的字数估下限：下限都超过了的，整份写的时候也轮不到它。

use std::collections::{BTreeSet, VecDeque};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History as Kept;
use miyu_kernel::id::{CallId, Seq};
use miyu_kernel::time::UtcOffset;
use miyu_store::log::Flow;
use miyu_store::sweep::{Ended, Plan, Settle, Sweep, Tuning};
use miyu_tool::{Log, Stop};

use super::entry::{self, Entry, Placeholders, Sender};
use super::page::{self, Footers, PAGE, Page};
use super::{NAME, Wanted, picked};

/// 边读边挑，交给 [`Sweep`] 收下定下来的那一截。
pub(super) struct Collect<'a> {
    placeholders: &'a Placeholders,
    wanted: &'a Wanted,
    offset: UtcOffset,
    /// 读到的每一条都记：派出去的任务（认别的会话），不留事件。
    notes: Kept,
    /// 读到了、还没轮到的人切权限级别，照序号。
    switches: VecDeque<Entry>,
    /// 她自己翻记录的调用编号。
    lookups: Lookups,
    /// 这一页。
    page: Gather,
}

/// 她自己翻记录的调用编号。
enum Lookups {
    /// 照定下来的回复一条条记。`bad` 是日志对不上调用编号的规矩：整份读。
    Streaming { seen: BTreeSet<CallId>, bad: bool },
    /// 整份读时照整份有效历史先算好的（同以前的读法）。
    Known(BTreeSet<CallId>),
}

/// 攒着的这一页。
enum Gather {
    /// 「读」：从头的几条，直到满了（`limit` 条，或者字数的下限超过了一页）；满了以后只看还有没有下一条（`more`）。
    Read {
        entries: Vec<(Entry, Sender)>,
        floor: usize,
        full: bool,
        more: bool,
    },
    /// 「找」：最新的几条命中（原文换成摘出来的一段），新的进来、轮不到的老的扔掉；命中一共几条。
    Found {
        hits: VecDeque<Hit>,
        floor: usize,
        total: usize,
    },
}

/// 「找」攒着的一条：条目（原文不留）、「谁」、摘出来的一段、这一行不算「谁」的字数。
struct Hit {
    entry: Entry,
    sender: Sender,
    snippet: String,
    floor: usize,
}

impl<'a> Collect<'a> {
    /// 照参数 `wanted` 挑，时刻照 `offset` 写。
    pub(super) fn new(
        placeholders: &'a Placeholders,
        wanted: &'a Wanted,
        offset: UtcOffset,
    ) -> Collect<'a> {
        let page = match wanted.words.is_empty() {
            true => Gather::Read {
                entries: Vec::new(),
                floor: 0,
                full: false,
                more: false,
            },
            false => Gather::Found {
                hits: VecDeque::new(),
                floor: 0,
                total: 0,
            },
        };
        Collect {
            placeholders,
            wanted,
            offset,
            notes: Kept::default(),
            switches: VecDeque::new(),
            lookups: Lookups::Streaming {
                seen: BTreeSet::new(),
                bad: false,
            },
            page,
        }
    }

    /// 日志对不上调用编号的规矩：这一遍挑的不作数，整份读。
    pub(super) fn bad(&self) -> bool {
        matches!(self.lookups, Lookups::Streaming { bad: true, .. })
    }

    /// 序号小于 `seq` 的人切权限级别都轮到了（没有的是剩下的都轮到了）：筛得进的交给这一页。
    fn switched_before(&mut self, seq: Option<Seq>) {
        while let Some(first) = self.switches.front()
            && seq.is_none_or(|seq| first.seq < seq.get())
        {
            if let Some(entry) = self.switches.pop_front()
                && picked(entry.seq, entry.at, entry.who, self.wanted)
            {
                self.offer(entry, Sender::Known(None));
            }
        }
    }

    /// 定下来的一条：是她自己翻记录的那几步的不算；算一条、筛得进的交给这一页。
    fn take(&mut self, event: &Event) {
        if !self.not_a_lookup(event) {
            return;
        }
        let Some((who, at)) = entry::kind_of(event) else {
            return;
        };
        if !picked(event.seq.get(), at, who, self.wanted) || !self.page.open() {
            return;
        }
        let text = entry::text_of(event, self.placeholders, NAME);
        if text.is_empty() {
            return;
        }
        let entry = Entry {
            seq: event.seq.get(),
            at,
            who,
            from: None,
            text,
        };
        let sender = entry::sender_of(event, self.placeholders);
        self.offer(entry, sender);
    }

    /// 记下回复里她自己翻记录的调用；工具结果是翻记录的那一次的交回 `false`。日志对不上规矩的记下来，之后都不挑。
    fn not_a_lookup(&mut self, event: &Event) -> bool {
        match (&mut self.lookups, &event.body) {
            (Lookups::Streaming { bad: true, .. }, _) => false,
            (Lookups::Streaming { seen, bad }, Body::MessageAssistant(reply)) => {
                for call in own_calls(&reply.blocks) {
                    if call.message() != event.seq {
                        *bad = true;
                        return false;
                    }
                    seen.insert(call);
                }
                true
            }
            (Lookups::Streaming { seen, bad }, Body::ToolResult(result)) => {
                if result.call_id.message() >= event.seq {
                    *bad = true;
                    return false;
                }
                !seen.contains(&result.call_id)
            }
            (Lookups::Known(known), Body::ToolResult(result)) => !known.contains(&result.call_id),
            _ => true,
        }
    }

    /// 算一条、筛得进的一条交给这一页。
    fn offer(&mut self, mut entry: Entry, sender: Sender) {
        let head = |entry: &Entry| entry.at.local_minute(self.offset);
        match &mut self.page {
            Gather::Read {
                entries,
                floor,
                full,
                more,
            } => {
                if *full {
                    *more = true;
                    return;
                }
                let gap = usize::from(!entries.is_empty());
                *floor += gap
                    + format!("#{} {} \n", entry.seq, head(&entry))
                        .chars()
                        .count()
                    + entry.text.chars().count()
                    + 1;
                entries.push((entry, sender));
                *full = entries.len() >= self.wanted.limit || *floor > PAGE;
            }
            Gather::Found { hits, floor, total } => {
                let Some(hit) = page::hit(&entry.text, &self.wanted.words) else {
                    return;
                };
                *total += 1;
                let snippet = page::snippet(&hit.text, hit.at, hit.len);
                let size = format!("#{} {} : {snippet}\n", entry.seq, head(&entry))
                    .chars()
                    .count();
                entry.text = String::new();
                *floor += size;
                hits.push_back(Hit {
                    entry,
                    sender,
                    snippet,
                    floor: size,
                });
                // 新的在前写：最新的 `limit` 条以外的，和后面的字数下限加起来已经超过一页的，都轮不到。
                while hits.len() > self.wanted.limit
                    || hits.len() > 1 && hits.front().is_some_and(|old| *floor - old.floor > PAGE)
                {
                    if let Some(old) = hits.pop_front() {
                        *floor -= old.floor;
                    }
                }
            }
        }
    }

    /// 挑完了：剩下的人切权限级别排进去，这一页几条的「谁」照整份日志记着的认，写成一页（「找」的一共几条是命中几条；「读」
    /// 的只数到这一页后面还有没有）。
    pub(super) fn page(mut self, footers: &Footers) -> Page {
        self.switched_before(None);
        let notes = &self.notes;
        let placeholders = self.placeholders;
        let resolve = |entry: &mut Entry, sender: Sender| {
            entry.from = match sender {
                Sender::Known(from) => from,
                Sender::Session(id) => notes.is_peer(&id).then(|| entry::peer(&id, placeholders)),
            };
        };
        match self.page {
            Gather::Read { entries, more, .. } => {
                let total = entries.len() + usize::from(more);
                let entries: Vec<Entry> = entries
                    .into_iter()
                    .map(|(mut entry, sender)| {
                        resolve(&mut entry, sender);
                        entry
                    })
                    .collect();
                page::read(&entries, total, self.wanted.limit, self.offset, footers)
            }
            Gather::Found { hits, total, .. } => {
                let found: Vec<(Entry, String)> = hits
                    .into_iter()
                    .map(|mut hit| {
                        resolve(&mut hit.entry, hit.sender);
                        (hit.entry, hit.snippet)
                    })
                    .collect();
                page::found(&found, total, self.wanted.limit, self.offset, footers)
            }
        }
    }
}

impl Gather {
    /// 还要不要下一条：「读」满了、也知道还有下一条的不要了。
    fn open(&self) -> bool {
        !matches!(
            self,
            Gather::Read {
                full: true,
                more: true,
                ..
            }
        )
    }
}

impl Settle for Collect<'_> {
    fn see(&mut self, event: &Event) {
        self.notes.note(event);
        if let Some(entry) = entry::switched(event, self.placeholders) {
            self.switches.push_back(entry);
        }
    }

    fn settle(&mut self, events: &[Event]) {
        for event in events {
            self.switched_before(Some(event.seq));
            self.take(event);
        }
    }
}

/// 回复里她自己翻记录的调用编号。
fn own_calls(blocks: &[Block]) -> impl Iterator<Item = CallId> + '_ {
    blocks.iter().filter_map(|block| match block {
        Block::ToolCall(call) if call.name == NAME => Some(call.call_id),
        _ => None,
    })
}

/// 读一遍挑出来：头一遍放早了的照下界再读一遍；骨架认不得、日志对不上调用编号的规矩的整份读（同以前的读法）。叫停了的交回
/// 空的；读不了的交回原因。`new` 每一遍造一个新的。
pub(super) fn collect<'a>(
    log: &Log,
    new: impl Fn() -> Collect<'a>,
    stop: &Stop,
    tuning: Tuning,
) -> Result<Option<Collect<'a>>, String> {
    let end = Seq::new(u64::MAX).unwrap_or(Seq::FIRST);
    let mut sweep = Sweep::new(Plan::guess(), tuning, new());
    log.read_range(Seq::FIRST, end, |events| {
        !stop.stopped() && matches!(sweep.feed(events), Flow::Go)
    })?;
    if stop.stopped() {
        return Ok(None);
    }
    let Some(last) = sweep.last else {
        return Ok(Some(sweep.settled));
    };
    sweep.finish();
    match sweep.ended {
        Ended::Done if !sweep.settled.bad() => return Ok(Some(sweep.settled)),
        Ended::Again if !sweep.settled.bad() => {
            let mut again = Sweep::new(Plan::exact(sweep.reaches), tuning, new());
            log.read_range(Seq::FIRST, last, |events| {
                !stop.stopped() && matches!(again.feed(events), Flow::Go)
            })?;
            if stop.stopped() {
                return Ok(None);
            }
            again.finish();
            if again.ended == Ended::Done && !again.settled.bad() {
                return Ok(Some(again.settled));
            }
        }
        _ => {}
    }
    whole(log, new(), stop, last)
}

/// 整份读到头一遍读到的第 `upto` 条（之后还在往后写的不要），整份交给留着一切的有效历史，再挑（以前的读法）。
fn whole<'a>(
    log: &Log,
    mut collect: Collect<'a>,
    stop: &Stop,
    upto: Seq,
) -> Result<Option<Collect<'a>>, String> {
    let mut kept = Kept::whole();
    log.read_range(Seq::FIRST, upto, |events| {
        if stop.stopped() {
            return false;
        }
        for event in events {
            collect.see(&event);
            kept.append(event);
        }
        true
    })?;
    if stop.stopped() {
        return Ok(None);
    }
    let known = kept
        .events()
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(own_calls(&reply.blocks)),
            _ => None,
        })
        .flatten()
        .collect();
    collect.lookups = Lookups::Known(known);
    collect.settle(kept.events());
    Ok(Some(collect))
}
