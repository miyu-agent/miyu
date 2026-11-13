//! 回合索引里的一条怎么从会话日志算出来（`docs/blueprint/memory.md`「怎么走」第一条，施工 R-2 上）：新版的「日记」，不另存，
//! 照日志派生（`docs/designs/17-记忆.md` L8）。
//!
//! - 只收人开的回合：`turn.started` 的 `trigger` 指的那条 `message.user` 是人（`person`、`external`）说的。别的 harness、
//!   子代理、别的会话发来的话开的，回报叫醒的，重启以后接着干的，没有 `trigger` 的，都不收（旧版自己造的回合进了日记）。
//! - 人生循环叫醒的回合也收（施工 LF-3，`By::is_lived_turn`）：一条的字只有她这一轮最后一条字不空的回复，不写叫醒的那段话；
//!   她一句没说的不收。别的循环、定时任务叫醒的照旧不收。增量、载入时铺回、照日志重放都走 [`TurnFeed::see`]，同一个判断。
//! - 一条的字：触发的那句人话，空一行，她这一轮最后一条字不空的回复；`turn.ended` 时交出来，哪种原因都收。
//! - 照记下的几条开的一轮（群里的 `session.respond`，`triggers`）：那几条人话一行一句都进，并进来的（`turn.joined`）接在
//!   后面；她这一轮没开口的不收，那是旁听（施工 R-13 再补，2026-10-12 项目主人：「把回复的那次记录进去就行，无论是插话还是
//!   直接触发」）。
//! - [`TurnFeed`] 是增量的：会话每落一批，一条条交给它。它只记还没触发回合的人话和正在进行的那一轮，结束了的不记，长会话在
//!   内存里不多背。所以 `turn.unreverted` 的那几轮它说不出字，交回 [`Change::Lost`]，执行器读整份日志交给 [`replay`]。

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use miyu_kernel::block::words;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::time::Timestamp;

/// 回合索引里的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnItem {
    /// 哪一轮。
    pub turn: TurnId,
    /// 字：触发的那句人话，空一行，她最后一条字不空的回复；只有一边有字的只写那一边。人生循环叫醒的只有她的回复（施工 LF-3）。
    pub text: String,
    /// 这一轮 `turn.started` 的时刻。
    pub at: Timestamp,
}

/// 看了一条事件以后要对回合库做的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// 放进这一条（键照 [`key`]）。
    Put(TurnItem),
    /// 拿掉这一轮的（撤销了）。撤销的每一轮都交，人开的、不是人开的都算：写的一方另埋一块墓碑，记忆的出处照它判
    /// （`memory.md` 第二条第 4 款）。
    Remove(TurnId),
    /// 这几轮恢复了：揭掉墓碑。人开的、不是人开的都交；有字可放的另交 `Put` 或 `Lost`，跟在它后面。
    Restored(Vec<TurnId>),
    /// 这几轮恢复了，可增量的时候不记得它们的字：读整份日志，照 [`replay`] 放回还在的。
    Lost(Vec<TurnId>),
}

/// 照一条条事件算回合索引（增量）。
#[derive(Debug, Default)]
pub struct TurnFeed {
    /// 还没触发回合的人话（连同人生循环叫醒的那段话，字记成空的）：序号、字。触发了一轮的那一条和它以前的都丢掉（排着的几条只有最后一条触发）。
    said: BTreeMap<Seq, String>,
    /// 正在进行的、人开的（或人生循环叫醒的）那一轮。
    open: Option<Open>,
    /// 整份过一遍时记得结束过的每一轮，恢复时照它放回（[`replay`]、[`TurnFeed::primed`]）；增量时没有。
    ended: Option<BTreeMap<TurnId, TurnItem>>,
}

/// 正在进行的、人开的（或人生循环叫醒的）一轮。
#[derive(Debug)]
struct Open {
    turn: TurnId,
    at: Timestamp,
    said: String,
    reply: String,
    /// 照记下的几条开的（群里的 `session.respond`）：她没开口的不收。
    ambient: bool,
}

impl TurnFeed {
    /// 载入会话时：照整份事件 `events` 把状态铺回来，交回接着用的增量 `TurnFeed` 和照到 `after` 以后的改动（库里已经有了
    /// 前面的；`after` 是空的，库里一条都没照过，交回全部）。
    pub fn primed(events: &[Event], after: Option<Seq>) -> (TurnFeed, Vec<Change>) {
        TurnFeed::primed_on(&[], events, after)
    }

    /// 同 [`TurnFeed::primed`]，分叉出来的支线（施工 R-13 三补）：先看一遍底子 `base`（父会话到分叉点为止的），只为认得那里
    /// 还没回的人话（支线那一轮的触发在底子里），底子里父会话的回合不交、不记；再照支线自己的 `events` 铺。照 [`Priming`]
    /// 一条条喂，和边读日志边喂的是同一份代码。
    pub fn primed_on(
        base: &[Event],
        events: &[Event],
        after: Option<Seq>,
    ) -> (TurnFeed, Vec<Change>) {
        let mut priming = Priming::default();
        for event in base {
            priming.base(event);
        }
        for event in events {
            priming.own(event);
        }
        priming.finish(after)
    }

    /// 看一条事件，交回要对回合库做的（照先后）。
    pub fn see(&mut self, event: &Event) -> Vec<Change> {
        match &event.body {
            // 人生循环叫醒的那段话也记下（才开得了一轮），记成空的：每次都是同一段推演方法，一条里只写她的回复（施工 LF-3）。
            // 她一句没说的两边都没字，照「两边都没字的不收」不收。
            Body::MessageUser(message) if event.by.is_lived_turn() => {
                let said = match event.by.is_person() {
                    true => words(&message.blocks),
                    false => String::new(),
                };
                self.said.insert(event.seq, said);
                Vec::new()
            }
            Body::TurnStarted(started) => {
                let said = started.trigger.and_then(|trigger| {
                    let said = match started.triggers.is_empty() {
                        true => self.said.remove(&trigger),
                        false => self.take(&started.triggers),
                    };
                    self.said.retain(|seq, _| *seq > trigger);
                    said
                });
                self.open = match (event.turn, said) {
                    (Some(turn), Some(said)) => Some(Open {
                        turn,
                        at: event.at,
                        said,
                        reply: String::new(),
                        ambient: !started.triggers.is_empty(),
                    }),
                    _ => None,
                };
                Vec::new()
            }
            Body::TurnJoined(joined) => {
                let more = self.take(&joined.triggers);
                if let (Some(open), Some(more)) = (
                    self.open
                        .as_mut()
                        .filter(|open| Some(open.turn) == event.turn),
                    more,
                ) {
                    open.said = [open.said.as_str(), more.as_str()]
                        .into_iter()
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                }
                Vec::new()
            }
            Body::MessageAssistant(message) => {
                if let Some(open) = self
                    .open
                    .as_mut()
                    .filter(|open| Some(open.turn) == event.turn)
                {
                    let reply = words(&message.blocks);
                    if !reply.is_empty() {
                        open.reply = reply;
                    }
                }
                Vec::new()
            }
            Body::TurnEnded(_) => {
                let Some(open) = self.open.take_if(|open| Some(open.turn) == event.turn) else {
                    return Vec::new();
                };
                if open.ambient && open.reply.is_empty() {
                    return Vec::new();
                }
                let text = [open.said, open.reply]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                if text.is_empty() {
                    return Vec::new();
                }
                let item = TurnItem {
                    turn: open.turn,
                    text,
                    at: open.at,
                };
                if let Some(ended) = self.ended.as_mut() {
                    ended.insert(item.turn, item.clone());
                }
                vec![Change::Put(item)]
            }
            Body::TurnReverted(reverted) => {
                reverted.turns.iter().copied().map(Change::Remove).collect()
            }
            Body::TurnUnreverted(unreverted) => {
                let restored = Change::Restored(unreverted.turns.clone());
                match &self.ended {
                    Some(ended) => std::iter::once(restored)
                        .chain(
                            unreverted
                                .turns
                                .iter()
                                .filter_map(|turn| ended.get(turn).cloned().map(Change::Put)),
                        )
                        .collect(),
                    None => vec![restored, Change::Lost(unreverted.turns.clone())],
                }
            }
            _ => Vec::new(),
        }
    }
}

impl TurnFeed {
    /// 拿走记下的几句人话 `seqs`（照序号），有字的一行一句接起来（只带了图的那句没有字，同只有一句时）；一句都不是人话的
    /// 没有。
    fn take(&mut self, seqs: &[Seq]) -> Option<String> {
        let said: Vec<String> = seqs
            .iter()
            .filter_map(|seq| self.said.remove(seq))
            .collect();
        (!said.is_empty()).then(|| {
            said.into_iter()
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
    }
}

/// 一条条铺回 [`TurnFeed`]（施工 V-37 下）：接上回合索引时边读日志边喂，整份事件不用同时在内存里。先喂底子
/// （[`Priming::base`]），再喂会话自己的（[`Priming::own`]），铺完照库里照到哪挑出改动（[`Priming::finish`]）。
/// [`TurnFeed::primed_on`] 就是照它走的。
#[derive(Debug, Default)]
pub struct Priming {
    feed: TurnFeed,
    /// 自己的每一条交回的改动，连同那一条的序号：照到哪要等铺完、开了回合库才知道，先都收着。
    changes: Vec<(Seq, Change)>,
}

impl Priming {
    /// 底子里的一条（同 [`TurnFeed::primed_on`] 的 `base`）：只看，不交改动、不记结束了的。底子要在自己的前面喂完。
    pub fn base(&mut self, event: &Event) {
        self.feed.see(event);
    }

    /// 会话自己的一条：记得结束过的每一轮（恢复时照它放回），交回的改动连同序号收着。
    pub fn own(&mut self, event: &Event) {
        if self.feed.ended.is_none() {
            self.feed.ended = Some(BTreeMap::new());
        }
        let seen = self.feed.see(event);
        self.changes
            .extend(seen.into_iter().map(|change| (event.seq, change)));
    }

    /// 铺完：交回接着用的增量 `TurnFeed`（不再记结束了的），和序号大于 `after` 的那几条交回的改动，照先后（`after` 是空的
    /// 全交，同 [`TurnFeed::primed_on`]）。
    pub fn finish(self, after: Option<Seq>) -> (TurnFeed, Vec<Change>) {
        let Priming { mut feed, changes } = self;
        feed.ended = None;
        let changes = changes
            .into_iter()
            .filter(|(seq, _)| after.is_none_or(|after| *seq > after))
            .map(|(_, change)| change)
            .collect();
        (feed, changes)
    }
}

/// 整份日志过一遍，交回最后还在的几条，照回合的先后：撤销了没恢复的不在，恢复了的在。照 [`Replay`] 一条条喂。
pub fn replay(events: &[Event]) -> Vec<TurnItem> {
    let mut replay = Replay::default();
    for event in events {
        replay.see(event);
    }
    replay.finish()
}

/// 同 [`replay`]，一条条喂（施工 V-37 下）：恢复了、增量不记得字的那几轮，执行器边读日志边喂，不整份读进内存。
#[derive(Debug)]
pub struct Replay {
    feed: TurnFeed,
    /// 到这一条为止还在的。
    kept: BTreeMap<TurnId, TurnItem>,
}

impl Default for Replay {
    fn default() -> Replay {
        Replay {
            feed: TurnFeed {
                ended: Some(BTreeMap::new()),
                ..TurnFeed::default()
            },
            kept: BTreeMap::new(),
        }
    }
}

impl Replay {
    /// 下一条。
    pub fn see(&mut self, event: &Event) {
        for change in self.feed.see(event) {
            match change {
                Change::Put(item) => {
                    self.kept.insert(item.turn, item);
                }
                Change::Remove(turn) => {
                    self.kept.remove(&turn);
                }
                Change::Restored(_) | Change::Lost(_) => {}
            }
        }
    }

    /// 喂完：最后还在的几条，照回合的先后。
    pub fn finish(self) -> Vec<TurnItem> {
        self.kept.into_values().collect()
    }
}

/// 回合库里的键：`会话编号/回合编号`，回合编号就是 `turn.started` 的序号。会话的全部条目都以 `会话编号/` 开头，删会话时
/// 照它拿掉。
pub fn key(session: &SessionId, turn: TurnId) -> String {
    format!("{session}/{}", turn.started().get())
}
