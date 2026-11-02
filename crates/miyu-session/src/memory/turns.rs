//! 一个会话的回合索引（施工 R-2 起，`docs/blueprint/memory.md` 第一条）：它的回合库（用到才开）、增量的 `TurnFeed`、
//! `UseFeed`；场所会话记下在哪个场所（施工 R-13 下），每个会话记下从哪来（是不是主会话，施工 E-6）。

use std::sync::{Arc, OnceLock};

use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId, TurnId, VenueId};
use miyu_recall::uses::{Use, UseFeed, since};
use miyu_recall::{Change, Priming, TurnFeed, TurnItem, key};
use miyu_store::recall::{Edit, Origin, RecallIndex, RecallIndexes, Room};

use super::MODULE;
use crate::TARGET;
use crate::agents::LOCAL;

/// 一个会话的回合索引：它的回合库（用到才开）、增量的 `TurnFeed`。
pub(crate) struct Turns {
    /// 会话编号：键的前一半，`marks` 里的来源。
    session: SessionId,
    /// 核心的登记，和这个会话的那一间：回合库照它开。
    recall: Arc<RecallIndexes>,
    room: Room,
    /// 开了的回合库：新造的会话到第一次真要写的时候才开，造会话不多一次开库、建表、同步。
    index: OnceLock<Arc<RecallIndex>>,
    /// 增量算。
    feed: TurnFeed,
    /// 用到了哪几条，增量算（施工 R-4 下）。
    uses: UseFeed,
    /// 场所会话在哪个场所（施工 R-13 下）：本机的没有。回合库里记一次（`places`），搜以前的对话照它挑。
    venue: Option<VenueId>,
    /// 记过场所了（这一回）。
    placed: OnceLock<()>,
    /// 这个会话从哪来（施工 E-6）：是不是主会话。回合库里记一次（`origins`），搜以前的对话照它判打不打得开。
    origin: Origin,
    /// 记过从哪来了（这一回）。
    originated: OnceLock<()>,
}

/// 一条条铺回一个会话的回合索引（施工 V-37 下）：接上时边读日志边喂，整份事件不进内存。先喂底子（父会话到分叉点为止的，
/// [`Laying::base`]），再照先后喂会话自己的（[`Laying::own`]），铺完交给 [`Turns::lay`] 照库里照到哪写。
pub(crate) struct Laying {
    /// 中途换过人格的，换的那一条（施工 P-13）：到它为止的（连同它）并进底子。
    floor: Option<Seq>,
    priming: Priming,
    uses: UseFeed,
    /// 换人格以后的每一条用到的，照先后：照到哪要等开了库才知道，先收着。
    used: Vec<Use>,
    /// 会话自己的最后一条。
    own: Option<Seq>,
    /// 换人格以后的最后一条：没有的不开库。
    mine: Option<Seq>,
}

impl Laying {
    /// 什么都还没喂；`floor` 照 [`super::recast::floor`]。
    pub(crate) fn new(floor: Option<Seq>) -> Laying {
        Laying {
            floor,
            priming: Priming::default(),
            uses: UseFeed::new(MODULE, miyu_tool::MEMORY_SEARCH),
            used: Vec::new(),
            own: None,
            mine: None,
        }
    }

    /// 底子里的一条（施工 R-13 三补）：只为认得那里还没回的人话，父会话的回合不在支线名下再记。要在自己的前面喂完。
    pub(crate) fn base(&mut self, event: &Event) {
        self.priming.base(event);
    }

    /// 会话自己的一条，照先后：换人格那一条以前的（连同它）同底子（`recast::split`），以后的铺回 `TurnFeed`、`UseFeed`。
    pub(crate) fn own(&mut self, event: &Event) {
        self.own = Some(event.seq);
        if self.floor.is_some_and(|floor| event.seq <= floor) {
            self.priming.base(event);
            return;
        }
        self.priming.own(event);
        self.used.extend(self.uses.see(event));
        self.mine = Some(event.seq);
    }

    /// 会话自己的最后一条：接上以后照它认交来的一批里铺过的。一条都没有的没有。
    pub(crate) fn upto(&self) -> Option<Seq> {
        self.own
    }
}

impl Turns {
    /// 会话 `session`（在场所 `venue`）的回合索引，放在 `room` 那一间：什么都还没铺，不开库。
    fn empty(
        recall: &Arc<RecallIndexes>,
        room: &Room,
        (session, origin): (&SessionId, Origin),
        venue: &VenueId,
    ) -> Turns {
        Turns {
            session: session.clone(),
            recall: Arc::clone(recall),
            room: room.clone(),
            index: OnceLock::new(),
            feed: TurnFeed::default(),
            uses: UseFeed::new(MODULE, miyu_tool::MEMORY_SEARCH),
            venue: (venue.as_str() != LOCAL).then(|| venue.clone()),
            placed: OnceLock::new(),
            origin,
            originated: OnceLock::new(),
        }
    }

    /// 接上会话 `session`（在场所 `venue`）的回合索引，放在 `room` 那一间，照一条条铺好的 `laying`（施工 V-37 下）。在阻塞
    /// 线程里调。
    ///
    /// 照回合库记着的照到哪以后的补上（一条都没照过的整份补）；换人格以后一条都没有的（新造的会话）不开库，`TurnFeed`、
    /// `UseFeed` 从空的起。库是铺完才开的：读日志读到一半读不了的，一点都不碰库（同原来整份读完再开）。`read` 照回合编号
    /// 读回那几轮的字（`Change::Lost`），铺的时候记得结束过的每一轮，交不出这一样，不会叫它。
    pub(crate) fn lay(
        recall: &Arc<RecallIndexes>,
        room: &Room,
        (session, origin): (&SessionId, Origin),
        venue: &VenueId,
        laying: Laying,
        read: impl FnOnce(&[TurnId]) -> Result<Vec<TurnItem>, String>,
    ) -> Turns {
        let mut turns = Turns::empty(recall, room, (session, origin), venue);
        let Laying {
            priming,
            uses,
            used,
            mine,
            ..
        } = laying;
        let Some(last) = mine else {
            return turns;
        };
        let mark = turns.mark();
        // 开了库的这时就记下从哪来（施工 E-6）：载入以后没有新回合的会话也补上，不等下一次写。
        turns.originate();
        let (feed, changes) = priming.finish(mark);
        turns.feed = feed;
        turns.uses = uses;
        let used = since(used, mark);
        if mark.is_none_or(|mark| mark < last) {
            turns.write(changes, &used, last, read);
        }
        turns
    }

    /// 照整份事件接上（补齐旧会话用，`backfill.rs`）：`events` 是整份事件，照它铺回 `TurnFeed`，照回合库记着的照到哪以后的
    /// 补上（一条都没照过的整份补）；新造的会话是空的，这时不开库。分叉出来的支线另交底子 `base`（父会话到分叉点为止的，
    /// 施工 R-13 三补）：只为认得那里还没回的人话，父会话的回合不在支线名下再记；别的会话是空的。载入时接上的边读边铺
    /// （[`Turns::lay`]，施工 V-37 下），写进库的一样，测试里拿这一个当准绳比。
    pub(crate) fn connect(
        recall: &Arc<RecallIndexes>,
        room: &Room,
        (session, origin): (&SessionId, Origin),
        venue: &VenueId,
        base: &[Event],
        events: &[Event],
    ) -> Option<Turns> {
        let mut turns = Turns::empty(recall, room, (session, origin), venue);
        let Some(last) = events.last() else {
            return Some(turns);
        };
        let mark = turns.mark();
        turns.originate();
        let (feed, changes) = TurnFeed::primed_on(base, events, mark);
        turns.feed = feed;
        let used = turns.uses.prime(events, mark);
        if mark.is_none_or(|mark| mark < last.seq) {
            turns.write(changes, &used, last.seq, |lost| {
                Ok(miyu_recall::replay(events)
                    .into_iter()
                    .filter(|item| lost.contains(&item.turn))
                    .collect())
            });
        }
        Some(turns)
    }

    /// 回合库里这个会话照到哪：读不了的当一条都没照过（记一行）。
    fn mark(&self) -> Option<Seq> {
        self.index()
            .mark(&self.session.to_string())
            .unwrap_or_else(|error| {
                tracing::warn!(target: TARGET, error = %error, "memory index not read");
                None
            })
    }

    /// 场所会话在回合库里记下它在哪个场所（施工 R-13 下）：这一回记一次；本机的不记。记不进的记一行，下一回再记。
    fn place(&self) {
        let Some(venue) = &self.venue else {
            return;
        };
        self.placed.get_or_init(|| {
            if let Err(error) = self.index().place(self.session.as_str(), venue.as_str()) {
                tracing::warn!(target: TARGET, error = %error, "memory index not updated");
            }
        });
    }

    /// 回合库记下这个会话从哪来（施工 E-6）：这一回记一次。记不进的记一行，下一回再记。
    fn originate(&self) {
        self.originated.get_or_init(|| {
            if let Err(error) = self.index().origin(self.session.as_str(), &self.origin) {
                tracing::warn!(target: TARGET, error = %error, "memory index not updated");
            }
        });
    }

    /// 这个会话的回合库：第一次用时照登记开（这一回第一次开的，登记记一行，[`super::Memory::new`]）。
    fn index(&self) -> &RecallIndex {
        self.index.get_or_init(|| self.recall.turns(&self.room).0)
    }

    /// 一批事件落了盘：交给 `TurnFeed`、`UseFeed`，照它们交回的改回合库，连同照到了这一批的最后一条。恢复的那几轮照回合
    /// 编号读回它们的字（`read`：照整份日志算，很少见）。
    pub(crate) fn advance(
        &mut self,
        events: &[Event],
        read: impl FnOnce(&[TurnId]) -> Result<Vec<TurnItem>, String>,
    ) {
        let Some(last) = events.last() else {
            return;
        };
        let changes: Vec<Change> = events
            .iter()
            .flat_map(|event| self.feed.see(event))
            .collect();
        let used: Vec<Use> = events
            .iter()
            .flat_map(|event| self.uses.see(event))
            .collect();
        self.write(changes, &used, last.seq, read);
    }

    /// 把改动、用到的写进回合库，照到 `last`。
    fn write(
        &self,
        changes: Vec<Change>,
        used: &[Use],
        last: Seq,
        read: impl FnOnce(&[TurnId]) -> Result<Vec<TurnItem>, String>,
    ) {
        // 用到的一次一行，键是 `会话/事件的序号`：删会话时照来源一起拿掉。
        let mut edits: Vec<Edit> = used
            .iter()
            .map(|used| Edit::Use {
                key: format!("{}/{}", self.session, used.seq.get()),
                memory: used.memory.to_string(),
                at: used.at,
            })
            .collect();
        let mut lost = Vec::new();
        for change in changes {
            match change {
                Change::Put(item) => edits.push(Edit::Put {
                    key: key(&self.session, item.turn),
                    text: item.text,
                    at: item.at,
                }),
                // 撤销的那一轮拿掉，再埋一块墓碑：记忆的出处在它里面的照它判死了（施工 R-3 上）。
                Change::Remove(turn) => {
                    let key = key(&self.session, turn);
                    edits.push(Edit::Remove { key: key.clone() });
                    edits.push(Edit::Bury { key });
                }
                Change::Restored(turns) => {
                    edits.extend(turns.into_iter().map(|turn| Edit::Unbury {
                        key: key(&self.session, turn),
                    }))
                }
                Change::Lost(turns) => lost.extend(turns),
            }
        }
        // 这一批没有要改的不写：照到的位置落在后面不要紧，载入时多铺一截、交回的还是空的（每落一批少写一次库）。
        if edits.is_empty() && lost.is_empty() {
            return;
        }
        if !lost.is_empty() {
            match read(&lost) {
                Ok(items) => edits.extend(items.into_iter().map(|item| Edit::Put {
                    key: key(&self.session, item.turn),
                    text: item.text,
                    at: item.at,
                })),
                Err(error) => {
                    // 读不回的不往前挪：下次载入照日志整份补。
                    tracing::warn!(target: TARGET, error = %error, "memory index not updated");
                    return;
                }
            }
        }
        self.place();
        self.originate();
        if let Err(error) = self.index().apply(&self.session.to_string(), &edits, last) {
            tracing::warn!(target: TARGET, error = %error, "memory index not updated");
        }
    }
}
