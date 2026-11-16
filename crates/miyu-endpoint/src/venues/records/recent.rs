//! 判官的记录窗（施工 V-44，`docs/construction/V-44-判官记录窗.md`；2026-11-03 项目主人批：每个开着的群会话常驻约 1 MB 换判官
//! CPU）：载入着的会话（群聊桥一直订阅着场所会话，它本来就在内存里）旁边留着判官回答要的那几样，追加的事件落了盘顺手更新
//! （[`miyu_session::Onlooker`]），判官问的时候照它答，不再每次把整份日志从头解一遍。
//!
//! 窗里留的（[`Lately`]）都照「留着一切的有效历史」（`History::whole`）算，和边读边记（`read.rs`）收的同一套：
//! - 读到的第一条（钉下的字照它找）；
//! - 记录（[`on_record`]）一共几条、最后 [`Caps::ring`] 条；
//! - 撤回（`venue.recalled`）：序号、平台编号、谁撤的，全留（超过 [`Caps::recalls`] 条的窗作废）；
//! - 引用能指到的编号（[`quoted_as`]）的哈希 → 最后一条的序号，最近 [`Caps::quotes`] 个（更早的挤掉）。
//!
//! 出厂的上限下一个会话的窗约 0.3 MB（V-44 施工单「实测」，heaptrack 量的）。
//!
//! 怎么建：会话载入以后判官头一回问（或者窗作废了以后头一回问），照旧走边读边记的那一遍，同一遍里顺手收一份到读到的最后一条
//! 为止的窗（[`Lately::settle`]）；读的时候又写进来的，照日志补读那一截再装上（[`install`]）。建的时候这一问照旧是整份读的
//! 答案。
//!
//! 什么时候照窗答（[`Lately::ask`]）：窗是好的；要判的那一条在留着的最后几条里（它以后的记录就都在窗里，之前一共几条照总数
//! 减出来）；要显示的几条都在窗里；引用的编号在窗里记着、记的那一条在要判的以前（不在窗里的照序号读回那一条，再对一遍编号），
//! 或者从没挤掉过、窗里没有就是日志里没有。别的一律退回边读边记的整份读（行为同以前）。
//!
//! 什么时候作废：追加的里有撤销、恢复、撤回排着的话（有效历史改了前面的，窗里收的可能不对了）；序号接不上；撤回多过上限。
//! 作废的窗当场放掉，下一问照整份读、顺手重建。

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use miyu_assemble::group::{Picked, Records, Window, on_record, quoted_as, records_from};
use miyu_assemble::{ForwardTexts, GroupChat};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{ExternalId, Seq};
use miyu_store::log::Flow;

use super::read::Log;
use crate::refusal::Refusal;
use crate::sweep::bones::hash;

/// 窗留多少（测试里调小，逼出挤掉、不够的几种）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Caps {
    /// 记录留最后几条：判官最多显示 100 条（`count` 100，或 `count + grow − 1` ≤ 99），多留的是要判的那一条落在后面几条
    /// 以前的余量（im 出厂四个判官同时问，问的时候又来了几条）。
    pub(crate) ring: usize,
    /// 引用能指到的编号留最近几个。
    pub(crate) quotes: usize,
    /// 撤回最多留几条，再多窗作废。
    pub(crate) recalls: usize,
}

impl Caps {
    /// 出厂的：记录 128 条（一条场所人话连正文约 0.5～1 KB）；编号 3584 个（哈希、序号各 8 字节：表一直在挤掉、记新的，
    /// 删了的格要等重排才腾出来，表长到 8192 格约 140 KB，先后约 57 KB）；撤回 2048 条。
    pub(crate) const SHIPPED: Caps = Caps {
        ring: 128,
        quotes: 3584,
        recalls: 2048,
    };
}

/// 一个会话的记录窗：装在会话的把手上（[`miyu_session::Handle::onlooker`]），和 actor 共用。
#[derive(Debug)]
pub(crate) struct Recent {
    caps: Caps,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// 建好的、还好的窗。
    lately: Option<Lately>,
    /// 有一问正在建：别的问照旧整份读，不一起建。
    building: bool,
    /// actor 交过来的最后一条（不管窗建没建好）。
    noted: Option<Seq>,
}

impl Recent {
    pub(crate) fn new(caps: Caps) -> Recent {
        Recent {
            caps,
            state: Mutex::new(State::default()),
        }
    }

    /// 留多少。
    pub(crate) fn caps(&self) -> Caps {
        self.caps
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 照窗答第 `msg` 条要的那几样（拷出来，不拿着锁渲染）；答不了的没有。答不了、窗也没有、没人在建的，这一问来建
    /// （`build` 写成真的，建完照 [`install`] 装上）。
    pub(crate) fn ask(&self, msg: Seq, window: Window) -> (Option<Snap>, bool) {
        let mut state = self.state();
        match &state.lately {
            Some(lately) => (lately.ask(msg, window), false),
            None if state.building => (None, false),
            None => {
                state.building = true;
                (None, true)
            }
        }
    }

    /// 建的那一问没建成（读不了、骨架认不得以外的退路都还能建；这里是连一遍都没读完的）：放开，下一问再建。
    pub(crate) fn abandon(&self) {
        self.state().building = false;
    }

    /// 有没有建好的窗（测试用）。
    #[cfg(test)]
    pub(crate) fn built(&self) -> bool {
        self.state().lately.is_some()
    }

    /// 建好的窗收到哪一条为止（测试用）。
    #[cfg(test)]
    pub(crate) fn upto(&self) -> Option<Seq> {
        self.state().lately.as_ref().and_then(|lately| lately.last)
    }

    /// 把编号 `id` 的哈希记成第 `seq` 条（测试用：照哈希撞了的样子造）。
    #[cfg(test)]
    pub(crate) fn misquote(&self, id: &str, seq: Seq) {
        if let Some(lately) = &mut self.state().lately {
            lately.quotes.insert(hash(id), seq);
        }
    }

    /// 第 `msg` 条照窗答不答得了（测试用，不动「在建」）。
    #[cfg(test)]
    pub(crate) fn answers(&self, msg: Seq, window: Window) -> bool {
        self.state()
            .lately
            .as_ref()
            .is_some_and(|lately| lately.ask(msg, window).is_some())
    }
}

impl miyu_session::Onlooker for Recent {
    fn appended(&self, events: &[Event]) {
        let Some(last) = events.last().map(|event| event.seq) else {
            return;
        };
        let mut state = self.state();
        state.noted = Some(last);
        if let Some(lately) = &mut state.lately {
            lately.note(events);
            if lately.broken {
                state.lately = None;
            }
        }
    }
}

/// 装上建好的窗：建的时候 actor 又交过来的（`noted` 比窗读到的新），照日志补读那一截再装；补读的时候又来了的再补。补读不了、
/// 补读里有作废的，不装。
pub(crate) fn install(recent: &Recent, mut lately: Lately, log: &mut dyn Log) {
    loop {
        let upto = {
            let mut state = recent.state();
            if lately.broken {
                state.building = false;
                return;
            }
            match state.noted {
                Some(noted) if lately.last.is_none_or(|last| noted > last) => noted,
                _ => {
                    state.lately = Some(lately);
                    state.building = false;
                    return;
                }
            }
        };
        let from = lately.last.map_or(Seq::FIRST, Seq::next);
        let read = log.range(from, Some(upto), &mut |events| {
            lately.note(&events);
            match lately.broken {
                true => Flow::Stop,
                false => Flow::Go,
            }
        });
        if read.is_err() || (!lately.broken && lately.last != Some(upto)) {
            recent.abandon();
            return;
        }
    }
}

/// 窗里留着的（见模块的说明）。也是边读边记那一遍顺手收的（[`Lately::settle`]）。
#[derive(Debug)]
pub(crate) struct Lately {
    caps: Caps,
    /// 收到哪一条为止（日志里的序号，不管在不在有效历史里）。
    pub(crate) last: Option<Seq>,
    /// 读到的第一条。
    pub(crate) first: Option<Arc<Event>>,
    /// 记录一共几条。
    before: usize,
    /// 记录的最后几条，照先后。
    ring: VecDeque<Arc<Event>>,
    /// 撤回，照先后：序号、平台编号、谁撤的。
    recalled: Vec<(Seq, String, ExternalId)>,
    /// 引用能指到的编号的哈希 → 最后一条的序号。
    quotes: HashMap<u64, Seq>,
    /// 记进 `quotes` 的先后，挤掉最早的用。
    order: VecDeque<(u64, Seq)>,
    /// 挤掉过：窗里没有的编号不能说日志里没有。
    evicted: bool,
    /// 作废了。
    pub(crate) broken: bool,
}

/// 照窗答一问要的，拷出来的。
#[derive(Debug)]
pub(crate) struct Snap {
    /// 读到的第一条。
    pub(crate) first: Option<Arc<Event>>,
    /// 要判的那一条。
    judged: Arc<Event>,
    /// 它以前的记录一共几条。
    before: usize,
    /// 要显示的那几条。
    shown: Vec<Arc<Event>>,
    /// 要显示的那几条里撤回了的：平台编号、谁撤的（它以前最后一条）。
    recalled: Vec<(String, ExternalId)>,
    /// 引用的那一条。
    pub(crate) quote: At,
}

/// 引用的那一条在哪。
#[derive(Debug)]
pub(crate) enum At {
    /// 不引用、日志里它以前没有那个编号的。
    None,
    /// 在窗里。
    Kept(Arc<Event>),
    /// 不在窗里：照序号读回来、再对一遍编号。
    Read(Seq),
}

impl Snap {
    /// 引用的编号（读回来的照它对）。
    pub(crate) fn reply_to(&self) -> Option<&str> {
        match &self.judged.body {
            Body::MessageUser(said) => said.venue.as_ref()?.reply_to.as_deref(),
            _ => None,
        }
    }

    /// 渲染：同 `read.rs` 的 `judge` 最后那一步。`quote` 是引用的那一条（读回来的照 [`Snap::reply_to`] 对过了）。
    pub(crate) fn render(
        &self,
        quote: Option<&Event>,
        window: Window,
        chat: &GroupChat,
        forward: Option<&ForwardTexts>,
    ) -> Result<Records, Refusal> {
        let last: Vec<&Event> = self.shown.iter().map(|event| &**event).collect();
        let recalled: BTreeMap<&str, &ExternalId> = self
            .recalled
            .iter()
            .map(|(msg, by)| (msg.as_str(), by))
            .collect();
        let quote = match &self.quote {
            At::Kept(event) => Some(&**event),
            At::Read(_) => quote,
            At::None => None,
        };
        let picked = Picked {
            judged: &self.judged,
            before: self.before,
            last: &last,
            recalled: &recalled,
            quote,
        };
        records_from(&picked, window, chat, forward).ok_or(Refusal::BAD_PARAMS)
    }
}

impl Lately {
    pub(crate) fn new(caps: Caps) -> Lately {
        Lately {
            caps,
            last: None,
            first: None,
            before: 0,
            ring: VecDeque::with_capacity(caps.ring + 1),
            recalled: Vec::new(),
            quotes: HashMap::new(),
            order: VecDeque::new(),
            evicted: false,
            broken: false,
        }
    }

    /// 边读边记那一遍定下来的一截（有效历史里的，照先后）：记录数一数、只拷可能留到最后的那几条，撤回、编号记下。
    pub(crate) fn settle(&mut self, events: &[Event]) {
        let mut last: Vec<&Event> = Vec::new();
        for event in events.iter().rev() {
            if on_record(event) {
                if last.len() < self.caps.ring {
                    last.push(event);
                }
                self.before += 1;
            }
        }
        for event in last.into_iter().rev() {
            self.ring.push_back(Arc::new(event.clone()));
            if self.ring.len() > self.caps.ring {
                self.ring.pop_front();
            }
        }
        for event in events {
            self.marks(event);
        }
    }

    /// 追加的一批（日志里的，照先后）：已经收过的跳过；撤销、恢复、撤回排着的话、序号接不上的作废。
    pub(crate) fn note(&mut self, events: &[Event]) {
        for event in events {
            if self.broken {
                return;
            }
            if self.last.is_some_and(|last| event.seq <= last) {
                continue;
            }
            if self.last.map_or(Seq::FIRST, Seq::next) != event.seq
                || matches!(
                    event.body,
                    Body::TurnReverted(_) | Body::TurnUnreverted(_) | Body::MessageWithdrawn(_)
                )
            {
                self.broken = true;
                return;
            }
            self.last = Some(event.seq);
            if on_record(event) {
                self.before += 1;
                self.ring.push_back(Arc::new(event.clone()));
                if self.ring.len() > self.caps.ring {
                    self.ring.pop_front();
                }
            }
            self.marks(event);
        }
    }

    /// 撤回、引用能指到的编号。
    fn marks(&mut self, event: &Event) {
        if let Body::VenueRecalled(recalled) = &event.body {
            if self.recalled.len() >= self.caps.recalls {
                self.broken = true;
            } else {
                self.recalled
                    .push((event.seq, recalled.msg.clone(), recalled.by.clone()));
            }
        }
        if let Some(id) = quoted_as(event) {
            // 先挤掉最早的再记：表和先后都不超过上限，不会为多一个翻倍。
            if self.order.len() >= self.caps.quotes
                && let Some((key, seq)) = self.order.pop_front()
            {
                if self.quotes.get(&key) == Some(&seq) {
                    self.quotes.remove(&key);
                }
                self.evicted = true;
            }
            if self.order.len() == self.order.capacity() {
                let want = (self.order.capacity() * 2).max(16).min(self.caps.quotes);
                self.order
                    .reserve_exact(want.saturating_sub(self.order.len()));
            }
            let key = hash(id);
            self.quotes.insert(key, event.seq);
            self.order.push_back((key, event.seq));
        }
    }

    /// 第 `msg` 条照 `window` 要的那几样（见模块的说明）；答不了的没有。
    fn ask(&self, msg: Seq, window: Window) -> Option<Snap> {
        if self.broken {
            return None;
        }
        let k = self
            .ring
            .binary_search_by_key(&msg, |event| event.seq)
            .ok()?;
        let before = self.before - (self.ring.len() - k);
        let shown = window.shown(before);
        if shown > k {
            return None;
        }
        let judged = Arc::clone(&self.ring[k]);
        let shown: Vec<Arc<Event>> = self.ring.range(k - shown..k).cloned().collect();
        let marks: BTreeMap<&str, &ExternalId> = self
            .recalled
            .iter()
            .take_while(|(seq, ..)| *seq < msg)
            .map(|(_, msg, by)| (msg.as_str(), by))
            .collect();
        let recalled = shown
            .iter()
            .filter_map(|event| match &event.body {
                Body::MessageUser(said) => said.venue.as_ref(),
                _ => None,
            })
            .filter_map(|venue| {
                marks
                    .get_key_value(venue.msg.as_str())
                    .map(|(msg, by)| ((*msg).to_string(), (*by).clone()))
            })
            .collect();
        let reply_to = match &judged.body {
            Body::MessageUser(said) => said
                .venue
                .as_ref()
                .and_then(|venue| venue.reply_to.as_deref()),
            _ => None,
        };
        let quote = match reply_to {
            None => At::None,
            Some(reply_to) => match self.quotes.get(&hash(reply_to)) {
                None if self.evicted => return None,
                None => At::None,
                Some(&seq) if seq >= msg => return None,
                Some(&seq) => match self.ring.binary_search_by_key(&seq, |event| event.seq) {
                    Ok(at) if quoted_as(&self.ring[at]) == Some(reply_to) => {
                        At::Kept(Arc::clone(&self.ring[at]))
                    }
                    Ok(_) => return None,
                    Err(_) => At::Read(seq),
                },
            },
        };
        Some(Snap {
            first: self.first.clone(),
            judged,
            before,
            shown,
            recalled,
            quote,
        })
    }
}
