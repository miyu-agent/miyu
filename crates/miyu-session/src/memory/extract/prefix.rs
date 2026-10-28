//! 抽取只读上次读到以后的那一截日志（施工 E-5 补，`docs/blueprint/memory.md` 第六条第 2 款）：一直热闹的会话每 20 轮抽一次
//! （施工 E-5），原来每次在阻塞线程里整份读一遍会话日志（大会话上近十兆，几十毫秒），和正在跑的回合抢 CPU。
//!
//! 拼这一段要的，除了上次抽到以后的事件，只有更早的日志里认出来的几样：名字对着平台编号、她被叫过的名字、最后的群名
//! （[`super::Voices`]）。渲染上次抽到以后的话照的是这几条自己（群里的一行、私聊的名字）；派出去的任务只管子代理、别的会话
//! 发来的话怎么包，那些话本来就不进这一段（不是人说的，第五条）。会话每落一批，这几样接着折进来（[`Prefix::fold`]）；批里有
//! 回合结束的，记下这一批落完日志照到哪（[`Landed`]）。抽的时候挑最后一处在上次抽到的那一条以前的，从那里读起
//! （`miyu_store::log::read_marked`），拼出来的请求和整份读的一个字节不差。
//!
//! 接不上的照旧整份读：核心刚起、会话刚换了人格（这时不知道更早的日志里有什么，整份读一次顺手接上，[`Prefix::of`]）；读起的
//! 那一处和日志对不上；日志里有撤销、恢复、撤回（整份读的名字照撤掉以后的有效历史认，折进来的没法撤，只能整份读）。

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;
use miyu_store::log::{Mark, read_marked};

use super::voices::{learn, owner_only};

/// 记几处读起的位置：一处一个回合结束，一直热闹的会话 20 轮抽一次，够挑。
const LANDINGS: usize = 64;

/// 还接不上时记下的批最多几条事件（载入以后整份读一次以前落的）：多了从头记，等下次整份读再接。
const PENDING: usize = 1024;

/// 日志里某一处以前认出来的几样：拼这一段、认名字要的。
#[derive(Debug, Clone)]
pub(crate) struct Prefix {
    /// 名字对着平台编号（同 [`super::Voices`] 的 `names`）。几处共用一份，变了才拷。
    pub(crate) names: Arc<BTreeMap<String, Option<String>>>,
    /// 她被叫过的名字，照 `fold` 过的（同 `Voices` 的 `me`）。
    pub(crate) me: Arc<BTreeSet<String>>,
    /// 最后一条带着群名的人话：序号和群名。
    pub(crate) place: Option<(Seq, String)>,
    /// 下一条该是几号：折到哪了。一条都没折的没有。
    next: Option<Seq>,
    /// 折过的里面没有撤销、恢复、撤回：有的话和整份读认的对不上。
    exact: bool,
}

impl Prefix {
    /// 一条都没折的。
    fn new() -> Prefix {
        Prefix {
            names: Arc::new(owner_only()),
            me: Arc::new(BTreeSet::new()),
            place: None,
            next: None,
            exact: true,
        }
    }

    /// 从日志第一条折到 `events` 最后一条。
    pub(crate) fn of(events: &[Event]) -> Prefix {
        let mut prefix = Prefix::new();
        prefix.fold(events);
        prefix
    }

    /// 接着折进 `events`（照先后、序号接着）。
    pub(crate) fn fold(&mut self, events: &[Event]) {
        self.exact &= !events.iter().any(unsettling);
        learn(events, |name, id| {
            if self.names.get(&name) != Some(&id) {
                Arc::make_mut(&mut self.names).insert(name, id);
            }
        });
        let called = crate::memory::her::called_in(events);
        if !called.is_subset(&self.me) {
            Arc::make_mut(&mut self.me).extend(called);
        }
        if let Some(place) = crate::memory::keeper::places(events).pop() {
            self.place = Some(place);
        }
        if let Some(last) = events.last() {
            self.next = Some(last.seq.next());
        }
    }
}

/// 改有效历史里更早的事件的那几种：撤销、恢复、撤回。
fn unsettling(event: &Event) -> bool {
    matches!(
        event.body,
        Body::TurnReverted(_) | Body::TurnUnreverted(_) | Body::MessageWithdrawn(_)
    )
}

/// 日志里的一处：落完一批照到哪（`mark`），和那以前认出来的（`prefix`）。
#[derive(Debug)]
pub(crate) struct Landed {
    pub(crate) mark: Mark,
    pub(crate) prefix: Prefix,
}

/// 抽取读回来的：上次抽到哪、读到的事件、从哪读起的。
#[derive(Debug)]
pub(crate) struct Read {
    pub(crate) after: Seq,
    pub(crate) events: Vec<Event>,
    pub(crate) from: From,
}

/// 从哪读起的。
#[derive(Debug)]
pub(crate) enum From {
    /// 从这一处读起：拼的时候接着它认出来的。
    Mark(Arc<Landed>),
    /// 整份读的；要接上的，顺手认出来的读到的末尾那一处。
    Start(Option<Landed>),
}

/// 记下的几处、还要不要整份读一次接上（[`super::Extractor::landings`]）：抽的时候交给读的那个阻塞线程。
#[derive(Debug)]
pub(crate) struct Landings {
    landings: Vec<Arc<Landed>>,
    seed: bool,
}

/// 会话日志在目录 `dir`，上次抽到第 `after` 条：`landings` 里有在它以前的，从最后那一处读起；没有的、对不上的、读起以后有撤销
/// 恢复撤回的整份读，还接不上的顺手认出读到的末尾那一处（[`Marks::seed`] 拿它接上）。
///
/// # Errors
///
/// 日志读不了：为什么。
pub(crate) fn read(dir: &Path, after: Seq, landings: &Landings) -> Result<Read, String> {
    let Landings { landings, seed } = landings;
    let seed = *seed;
    if let Some(landed) = landings
        .iter()
        .rev()
        .find(|landed| landed.mark.next.get() <= after.get().saturating_add(1))
    {
        let mut events = Vec::new();
        let read = read_marked(dir, Some(&landed.mark), |segment| {
            gather(&mut events, segment);
            true
        })
        .map_err(|error| error.to_string())?;
        let fits = read.is_some()
            && events
                .first()
                .is_none_or(|first| first.seq == landed.mark.next)
            && !events.iter().any(unsettling);
        if fits {
            return Ok(Read {
                after,
                events,
                from: From::Mark(Arc::clone(landed)),
            });
        }
    }
    let mut events = Vec::new();
    let end = read_marked(dir, None, |segment| {
        gather(&mut events, segment);
        true
    })
    .map_err(|error| error.to_string())?;
    let seeded = end.filter(|_| seed).map(|mark| Landed {
        mark,
        prefix: Prefix::of(&events),
    });
    Ok(Read {
        after,
        events,
        from: From::Start(seeded),
    })
}

/// 一段段收进一整份：头一段整个搬过来。
fn gather(events: &mut Vec<Event>, segment: Vec<Event>) {
    if events.is_empty() {
        *events = segment;
    } else {
        events.extend(segment);
    }
}

/// 一个会话折到哪、记下的几处（[`super::Extractor`] 里放着）。
#[derive(Debug, Default)]
pub(crate) struct Marks {
    /// 从日志第一条一路折过来的；不知道更早的日志里有什么的时候没有（核心刚起、刚换了人格：等整份读一次接上）。
    running: Option<Prefix>,
    /// 落完了回合结束的那一批的几处，旧的在前。
    landings: VecDeque<Arc<Landed>>,
    /// 这以后落的批折进 `running` 时，里面有回合结束的：落完记一处。
    ended: bool,
    /// 还接不上时落的批：第一批的头一条（从它起没漏）、事件。
    pending: Option<Seq>,
    held: Vec<Event>,
    /// 日志里有撤销、恢复、撤回：接不上，一直整份读，不再记批。
    never: bool,
}

impl Marks {
    /// 会话落了一批 `events`（落盘以前，照先后）。`keep` 是这个会话抽不抽：不抽的不记还接不上时落的批。
    pub(crate) fn heard(&mut self, events: &[Event], keep: bool) {
        if self.never || events.is_empty() {
            return;
        }
        if events.iter().any(unsettling) {
            *self = Marks {
                never: true,
                ..Marks::default()
            };
            return;
        }
        if let Some(running) = &mut self.running {
            running.fold(events);
            self.ended |= events
                .iter()
                .any(|event| matches!(event.body, Body::TurnEnded(_)));
            return;
        }
        if !keep {
            return;
        }
        if self.held.len() + events.len() > PENDING {
            self.held.clear();
            self.pending = None;
        }
        self.pending.get_or_insert(events[0].seq);
        self.held.extend_from_slice(events);
    }

    /// 那一批落完了，日志照到 `mark`：折到了这里、里面有回合结束的，记一处。
    pub(crate) fn landed(&mut self, mark: Mark) {
        let Some(running) = &self.running else {
            return;
        };
        // 折到的和日志照到的对不上（有没经这里的一批）：不知道了，等整份读一次再接上。
        if running.next.is_some_and(|next| next != mark.next) {
            *self = Marks::default();
            return;
        }
        if !std::mem::take(&mut self.ended) {
            return;
        }
        let landed = Landed {
            mark,
            prefix: running.clone(),
        };
        self.push(landed);
    }

    fn push(&mut self, landed: Landed) {
        if self.landings.len() == LANDINGS {
            self.landings.pop_front();
        }
        self.landings.push_back(Arc::new(landed));
    }

    /// 记下的几处、还要不要整份读一次接上：抽的时候在阻塞线程里挑。
    pub(crate) fn landings(&self) -> Landings {
        Landings {
            landings: self.landings.iter().cloned().collect(),
            seed: self.wants_seed(),
        }
    }

    /// 还接不上、等整份读一次接上。
    fn wants_seed(&self) -> bool {
        self.running.is_none() && !self.never
    }

    /// 整份读回来、认出了读到的末尾那一处 `landed`：接上（读的时候又落了的批照记下的折进去）。读到的里面有撤销、恢复、撤回的
    /// 一直整份读；读的时候落的批没记全的（记下的太多、从头记了），等下次。
    pub(crate) fn seed(&mut self, landed: Landed) {
        if !self.wants_seed() {
            return;
        }
        if !landed.prefix.exact {
            *self = Marks {
                never: true,
                ..Marks::default()
            };
            return;
        }
        if self
            .pending
            .is_some_and(|first| first.get() > landed.mark.next.get())
        {
            return;
        }
        let mut running = landed.prefix.clone();
        let later: Vec<Event> = std::mem::take(&mut self.held)
            .into_iter()
            .filter(|event| event.seq >= landed.mark.next)
            .collect();
        self.pending = None;
        running.fold(&later);
        self.running = Some(running);
        self.push(landed);
    }
}

#[cfg(test)]
mod tests;
