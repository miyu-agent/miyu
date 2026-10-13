//! 回合索引（施工 R-2 上，`docs/blueprint/memory.md`「怎么走」第一条）：主会话每落一批，把结束了的人开的回合放进这个人格的
//! 回合库，撤销的拿掉、恢复的读回，载入时照整份事件补上落下的。撤销的每一轮另埋一块墓碑、恢复的揭掉，记忆的出处照它判
//! 活不活（施工 R-3 上，第二条第 4 款）。记忆用到过的每一次（施工 R-4 下，第八条）跟着同一条路写进同一个回合库：照到的位置
//! 一份，载入、补齐旧会话一起补。怎么算是纯逻辑（`miyu_recall::TurnFeed`、`miyu_recall::uses`），这里只管接线和写库。
//!
//! 回合库是派生的：更新失败记一行 `WARN memory index not updated`，会话照常；照到的位置没往前挪，下次载入照日志补。

mod backfill;
mod extract;
mod follow;
mod keeper;
mod listener;
mod merge;
mod owner;
mod past;
mod port;
mod recall;
mod restore;
mod summary;
mod used;
mod vectors;

pub use extract::{ExtractTexts, Extraction};
pub(crate) use extract::{Extractor, Job, Voices, plan};
pub use follow::Following;
pub use keeper::{Filter, Keeper, Stamp};
pub use listener::Listener;
pub use merge::{Dreamed, MergeTexts, NotDreamed};
pub(crate) use owner::called;

/// 记忆模块的编号：注入、抽取记下的 `by` 都是它（施工 R-4 上、R-6 上）。
pub(crate) const MODULE: &str = "memory";
pub(crate) use port::Calls;
pub use recall::RecallTexts;
pub use summary::SummaryTexts;
pub use vectors::{LOCAL as EMBED_LOCAL, Query, Using, Vectors};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use miyu_kernel::event::{Event, SessionCreated};
use miyu_kernel::id::{AccountId, SessionId, VenueId};
use miyu_policy::memory::MemoryScope;
use miyu_recall::uses::{Use, UseFeed};
use miyu_recall::{Change, TurnFeed, key, replay};
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::{Edit, Opened, RecallIndex, RecallIndexes, Room};
use miyu_store::root::DataRoot;

use crate::TARGET;
use crate::agents::LOCAL;

/// 核心一份的记忆（施工 R-3 中）：回合库的登记、记忆日志的登记。会话表交给每个会话，主会话照它更新回合索引、给三件工具造
/// 端口。
#[derive(Debug)]
pub struct Memory {
    /// 回合库的登记（施工 R-2 上）。
    pub turns: Arc<RecallIndexes>,
    /// 记忆日志的登记（施工 R-3 上）。
    pub logs: Arc<MemoryLogs>,
    /// 常驻的摘要那一块的字（施工 R-4 上）：读不出来的（安装坏了）这个核心不交摘要。
    pub summary: Option<SummaryTexts>,
    /// 照意思找的那一路（施工 R-5 下）：核心起来时接上（[`Memory::give_vectors`]），没接的只照关键词找。
    vectors: OnceLock<Arc<Vectors>>,
    /// 抽取要的几样（施工 R-6 上）：核心起来时交（[`Memory::give_extraction`]），没交的不抽。
    extraction: OnceLock<Extraction>,
    /// 联想要的几样（施工 R-8）：核心起来时交（[`Memory::give_recall`]），没交的不联想。
    recall: OnceLock<RecallTexts>,
    /// 人格记忆这个软件包这时装着没有（施工 R-10）：开着的会话照它交不交摘要、抽不抽。
    installed: AtomicBool,
    /// 合并（施工 R-7 上）：哪几间在合、同一批失败了几次。
    merges: merge::Merges,
}

impl Memory {
    /// 数据根 `root` 上一份空的记忆：两份登记，用到哪一间才开。回合库第一次开时记一行运行日志；人格那一间是新建的、重建过的，
    /// 起一个后台线程补齐这个账号的旧会话（施工 R-2 下，`memory/backfill.rs`）。`summary` 是常驻的摘要那一块的字（施工
    /// R-4 上），没有的不交摘要。
    pub fn new(root: &DataRoot, summary: Option<SummaryTexts>) -> Arc<Memory> {
        let turns = Arc::new(RecallIndexes::new(root));
        let recall = Arc::downgrade(&turns);
        let at = root.clone();
        let registered = turns.when_opened(Box::new(move |room, opened| {
            log_opened(room, opened);
            let fresh = matches!(opened, Opened::Created | Opened::Rebuilt(_));
            if !fresh || !matches!(room, Room::Persona { .. }) {
                return;
            }
            // 登记被放下了（核心在退出）就不补：下次开库时照样是新建的、重建的，再补。
            let Some(recall) = recall.upgrade() else {
                return;
            };
            let (room, root) = (room.clone(), at.clone());
            let spawned = std::thread::Builder::new()
                .name("memory-backfill".to_string())
                .spawn(move || backfill::backfill(&recall, &root, &room));
            if let Err(error) = spawned {
                tracing::warn!(target: TARGET, error = %error, "memory index not backfilled");
            }
        }));
        debug_assert!(registered, "一份新的登记只登记这一次");
        Arc::new(Memory {
            turns,
            logs: Arc::new(MemoryLogs::new(root)),
            summary,
            vectors: OnceLock::new(),
            extraction: OnceLock::new(),
            recall: OnceLock::new(),
            installed: AtomicBool::new(true),
            merges: merge::Merges::default(),
        })
    }
}

impl Memory {
    /// 接上照意思找的那一路（施工 R-5 下）：核心起来、找好小程序和缓存目录以后接一次；接过的再接不算，交回 `false`。
    pub fn give_vectors(&self, vectors: Arc<Vectors>) -> bool {
        self.vectors.set(vectors).is_ok()
    }

    /// 照意思找的那一路：没接的没有。
    pub fn vectors(&self) -> Option<&Arc<Vectors>> {
        self.vectors.get()
    }

    /// 交上抽取要的几样（施工 R-6 上）：核心起来、造好一次性入口以后交一次；交过的再交不算，交回 `false`。
    pub fn give_extraction(&self, extraction: Extraction) -> bool {
        self.extraction.set(extraction).is_ok()
    }

    /// 抽取要的几样：没交的没有，不抽。
    pub(crate) fn extraction(&self) -> Option<&Extraction> {
        self.extraction.get()
    }

    /// 交上联想要的几样（施工 R-8）：核心起来时读好资源目录交一次；交过的再交不算，交回 `false`。
    pub fn give_recall(&self, recall: RecallTexts) -> bool {
        self.recall.set(recall).is_ok()
    }

    /// 联想要的几样：没交的没有，不联想。
    pub(crate) fn recall(&self) -> Option<&RecallTexts> {
        self.recall.get()
    }

    /// 人格记忆这个软件包装没装（施工 R-10，`memory.md` 第十一条）：核心起来时照清单设一次，装卸以后照新的清单设（卸包在动
    /// 文件以前）。没设过的当装着。没装的时候开着的会话不交摘要、不抽；磁盘上的不动，装回来接着用。
    pub fn set_installed(&self, installed: bool) {
        self.installed.store(installed, Ordering::SeqCst);
    }

    /// 人格记忆这个软件包这时装着没有（[`Memory::set_installed`]）。
    pub fn installed(&self) -> bool {
        self.installed.load(Ordering::SeqCst)
    }
}

/// 接上会话 `session` 的记忆：回合索引（[`Turns::connect`]）和三件工具的端口（[`Calls`]）。范围是 `off` 的（子会话、
/// `--no-memory`）不接，核心没交记忆的（测试里自己造的）没有。在阻塞线程里调。
///
/// 记忆放在哪照范围（`memory.md`「范围」）：跟着人格的在记忆账号 `account` 的那一间，只在这个会话里的在会话自己的目录
/// 里（属主 `owner` 的）。听众照属主，不照记忆账号：出厂人格的记忆归属主，系统账号开的会话归管理员，听的人还是属主。
///
/// 场所会话的回合也进回合索引，回合库记下它在哪个场所（施工 R-13 下，`memory.md` 第一条第 1 款）：搜以前的对话照谁在听挑，
/// 一个场所里只搜得到这个场所的（第九条）。原来（施工 R-2 再补）场所会话不进、载入时拿掉，回合库的条目那时还没有场所。
/// `events` 是底子（分叉出来的支线才有，施工 R-13 三补）和这个会话自己的整份事件。
#[expect(
    clippy::too_many_arguments,
    reason = "接上记忆要的几样各是一样，拼成一个结构体只为过这一条"
)]
pub(crate) fn connect(
    memory: Option<&Arc<Memory>>,
    scope: MemoryScope,
    account: &AccountId,
    owner: &AccountId,
    persona: &str,
    session: &SessionId,
    venue: &VenueId,
    events: (&[Event], &[Event]),
) -> (Option<Turns>, Option<Calls>) {
    let room = match scope {
        MemoryScope::Persona => Room::persona(account, persona),
        MemoryScope::Session => Room::session(owner, session),
        MemoryScope::Off => return (None, None),
    };
    let Some(memory) = memory else {
        return (None, None);
    };
    let turns = Turns::connect(&memory.turns, &room, session, venue, events.0, events.1);
    let listener = Listener::of(account, owner, venue);
    (
        turns,
        Some(Calls::new(memory, room, listener, owner, session)),
    )
}

/// 会话的记忆的范围：子会话（`child`）不管交的、快照里的是什么都是 `off`（17 第二节，以前造的子会话快照里没有这一格）；
/// 主会话照交的 `given`。分叉出来的支线不算子会话（[`subagent`]，施工 R-13 三补）。
pub(crate) fn scope(child: bool, opened: bool, given: MemoryScope) -> MemoryScope {
    if child || !opened {
        MemoryScope::Off
    } else {
        given
    }
}

/// 会话是不是派出去的子代理：有父会话、不是分叉出来的（施工 R-13 三补）。分叉出来的支线（群里第二个人 @ 她时分的，C-8）照
/// 父会话的记忆范围：群里她也照样记、照样想起（2026-10-11 项目主人定），支线也是。
pub(crate) fn subagent(created: &SessionCreated) -> bool {
    created.parent.is_some() && created.forked.is_none()
}

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
}

impl Turns {
    /// 接上会话 `session`（在场所 `venue`）的回合索引，放在 `room` 那一间。在阻塞线程里调。
    ///
    /// `events` 是载入时读到的整份事件：照它铺回 `TurnFeed`，照回合库记着的照到哪以后的补上（一条都没照过的整份补）；
    /// 新造的会话是空的，这时不开库。分叉出来的支线另交底子 `base`（父会话到分叉点为止的，施工 R-13 三补）：只为认得那里
    /// 还没回的人话，父会话的回合不在支线名下再记；别的会话是空的。
    pub(crate) fn connect(
        recall: &Arc<RecallIndexes>,
        room: &Room,
        session: &SessionId,
        venue: &VenueId,
        base: &[Event],
        events: &[Event],
    ) -> Option<Turns> {
        let mut turns = Turns {
            session: session.clone(),
            recall: Arc::clone(recall),
            room: room.clone(),
            index: OnceLock::new(),
            feed: TurnFeed::default(),
            uses: UseFeed::new(MODULE, miyu_tool::MEMORY_SEARCH),
            venue: (venue.as_str() != LOCAL).then(|| venue.clone()),
            placed: OnceLock::new(),
        };
        let Some(last) = events.last() else {
            return Some(turns);
        };
        let mark = turns
            .index()
            .mark(&session.to_string())
            .unwrap_or_else(|error| {
                tracing::warn!(target: TARGET, error = %error, "memory index not read");
                None
            });
        let (feed, changes) = TurnFeed::primed_on(base, events, mark);
        turns.feed = feed;
        let used = turns.uses.prime(events, mark);
        if mark.is_none_or(|mark| mark < last.seq) {
            turns.write(changes, &used, last, || Ok(events.to_vec()));
        }
        Some(turns)
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

    /// 这个会话的回合库：第一次用时照登记开（这一回第一次开的，登记记一行，[`Memory::new`]）。
    fn index(&self) -> &RecallIndex {
        self.index.get_or_init(|| self.recall.turns(&self.room).0)
    }

    /// 一批事件落了盘：交给 `TurnFeed`、`UseFeed`，照它们交回的改回合库，连同照到了这一批的最后一条。恢复的那几轮要读
    /// 整份日志（`read`，很少见）。
    pub(crate) fn advance(
        &mut self,
        events: &[Event],
        read: impl FnOnce() -> Result<Vec<Event>, String>,
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
        self.write(changes, &used, last, read);
    }

    /// 把改动、用到的写进回合库，照到 `last`。
    fn write(
        &self,
        changes: Vec<Change>,
        used: &[Use],
        last: &Event,
        read: impl FnOnce() -> Result<Vec<Event>, String>,
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
            match read() {
                Ok(events) => edits.extend(
                    replay(&events)
                        .into_iter()
                        .filter(|item| lost.contains(&item.turn))
                        .map(|item| Edit::Put {
                            key: key(&self.session, item.turn),
                            text: item.text,
                            at: item.at,
                        }),
                ),
                Err(error) => {
                    // 读不回的不往前挪：下次载入照日志整份补。
                    tracing::warn!(target: TARGET, error = %error, "memory index not updated");
                    return;
                }
            }
        }
        self.place();
        if let Err(error) = self
            .index()
            .apply(&self.session.to_string(), &edits, last.seq)
        {
            tracing::warn!(target: TARGET, error = %error, "memory index not updated");
        }
    }
}

/// 回合库这一回第一次开：新建的记 `INFO`，重建的、用不了的记 `WARN`（照会话列表的索引的说法）。哪一间写成
/// `persona <账号>/<人格>`、`session <账号>/<会话>`（施工 R-2 下：开库的四处都从登记过，原来只有会话写回合那一处记）。
fn log_opened(room: &Room, opened: &Opened) {
    let room = room.to_string();
    let room = room.as_str();
    match opened {
        Opened::Kept => {}
        Opened::Created => tracing::info!(target: TARGET, room, "memory index created"),
        Opened::Rebuilt(why) => {
            tracing::warn!(target: TARGET, room, reason = %why, "memory index rebuilt");
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, room, error = %error, "memory index unusable");
        }
    }
}
