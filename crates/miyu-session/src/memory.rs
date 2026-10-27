//! 回合索引（施工 R-2 上，`docs/blueprint/memory.md`「怎么走」第一条）：主会话每落一批，把结束了的人开的回合放进这个人格的
//! 回合库，撤销的拿掉、恢复的读回，载入时照整份事件补上落下的。撤销的每一轮另埋一块墓碑、恢复的揭掉，记忆的出处照它判
//! 活不活（施工 R-3 上，第二条第 4 款）。记忆用到过的每一次（施工 R-4 下，第八条）跟着同一条路写进同一个回合库：照到的位置
//! 一份，载入、补齐旧会话一起补。怎么算是纯逻辑（`miyu_recall::TurnFeed`、`miyu_recall::uses`），这里只管接线和写库。
//!
//! 回合库是派生的：更新失败记一行 `WARN memory index not updated`，会话照常；照到的位置没往前挪，下次载入照日志补。

mod backfill;
mod cards;
mod dreaming;
mod extract;
mod follow;
mod her;
mod keeper;
mod known;
mod lane;
mod listener;
mod merge;
mod owner;
mod past;
mod port;
mod recall;
mod recast;
mod restore;
mod sessions;
mod summary;
mod tag;
mod turns;
mod used;
mod vectors;

pub use cards::CardRefused;
pub use dreaming::{DreamFeed, Dreaming};
pub use extract::{ExtractTexts, Extraction};
pub(crate) use extract::{Extractor, Job, Voices, ended_only, plan, remembered_since};
pub use follow::Following;
pub use keeper::{Filter, Keeper, Stamp};
pub(crate) use lane::{Laid, Source, TurnLane};
pub use listener::Listener;
pub(crate) use merge::Hush;
pub use merge::{Dreamed, MergeTexts, NotDreamed};
pub(crate) use owner::called;

/// 记忆模块的编号：注入、抽取记下的 `by` 都是它（施工 R-4 上、R-6 上）。
pub(crate) const MODULE: &str = "memory";
pub(crate) use port::Calls;
pub use recall::RecallTexts;
pub(crate) use recast::{RoomCell, room_cell};
pub use sessions::{DreamBatch, DreamResult, DreamRoom, DreamSessions, DreamSpec};
pub use summary::SummaryTexts;
pub use tag::TagTexts;
pub(crate) use turns::Turns;
pub use vectors::{LOCAL as EMBED_LOCAL, Query, Using, Vectors};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use miyu_kernel::event::SessionCreated;
use miyu_kernel::id::{AccountId, SessionId, VenueId};
use miyu_policy::memory::MemoryScope;
use miyu_store::memory::MemoryLogs;
use miyu_store::personas::Personas;
use miyu_store::recall::{LogSalt, NO_PERSONA, Opened, Origin, RecallIndexes, Room};
use miyu_store::root::DataRoot;

use crate::TARGET;

/// 核心一份的记忆（施工 R-3 中）：回合库的登记、记忆日志的登记。会话表交给每个会话，主会话照它更新回合索引、给三件工具造
/// 端口。
#[derive(Debug)]
pub struct Memory {
    /// 回合库的登记（施工 R-2 上）。
    pub turns: Arc<RecallIndexes>,
    /// 数据根（施工 E-1 下）：整理合人卡时读出处那几轮的会话日志，认出她记的名字是谁。
    root: DataRoot,
    /// 运行日志里写哪一间的盐（施工 LG-1 下）：数据根的 `state/log-salt`，读不出、写不成的这次起来另造一份只在内存里的。
    salt: LogSalt,
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
    /// 回合索引在后台写（施工 V-9）。
    lanes: lane::Lanes,
    /// 补齐旧会话的后台线程（施工 R-2 下）：[`Memory::settle`] 也等它们。
    backfills: Arc<backfill::Backfills>,
    /// 整理的时候她正在写什么（施工 R-21）：一个核心一路广播。
    dreams: dreaming::Dreams,
    /// 整理会话（施工 D-1 上）：核心起来时装上会话表（[`Memory::give_dream_sessions`]），没装的照旧走一次性入口。
    dream_sessions: OnceLock<Arc<dyn DreamSessions>>,
    /// 每一间她叫什么（施工 E-1 修）：认识的人不记她自己。
    selves: her::Selves,
}

impl Memory {
    /// 数据根 `root` 上一份空的记忆：两份登记，用到哪一间才开。回合库第一次开时记一行运行日志；人格那一间是新建的、重建过的，
    /// 起一个后台线程补齐这个账号的旧会话（施工 R-2 下，`memory/backfill.rs`）。`summary` 是常驻的摘要那一块的字（施工
    /// R-4 上），没有的不交摘要。
    pub fn new(root: &DataRoot, summary: Option<SummaryTexts>) -> Arc<Memory> {
        let turns = Arc::new(RecallIndexes::new(root));
        let salt = LogSalt::load(root).unwrap_or_else(|error| {
            tracing::warn!(target: TARGET, error = %error.kind(), "memory log salt not kept");
            LogSalt::fresh()
        });
        let opening = salt.clone();
        let recall = Arc::downgrade(&turns);
        let at = root.clone();
        let backfills = Arc::new(backfill::Backfills::default());
        let filling = Arc::clone(&backfills);
        let registered = turns.when_opened(Box::new(move |room, opened| {
            let id = opening.room_id(room);
            log_opened(room, &id, opened);
            let fresh = matches!(opened, Opened::Created | Opened::Rebuilt(_));
            if !fresh || !matches!(room, Room::Persona { .. }) {
                return;
            }
            // 登记被放下了（核心在退出）就不补：下次开库时照样是新建的、重建的，再补。
            let Some(recall) = recall.upgrade() else {
                return;
            };
            filling.spawn(recall, at.clone(), room.clone(), id);
        }));
        debug_assert!(registered, "一份新的登记只登记这一次");
        Arc::new(Memory {
            turns,
            root: root.clone(),
            salt,
            logs: Arc::new(MemoryLogs::new(root)),
            summary,
            vectors: OnceLock::new(),
            dream_sessions: OnceLock::new(),
            extraction: OnceLock::new(),
            recall: OnceLock::new(),
            installed: AtomicBool::new(true),
            merges: merge::Merges::default(),
            lanes: lane::Lanes::default(),
            backfills,
            dreams: dreaming::Dreams::default(),
            selves: her::Selves::default(),
        })
    }
}

impl Memory {
    /// 运行日志里这一间的短号（施工 LG-1 下）：照这个数据根的盐算，同一间每次一样；和 [`Room::kind`] 一起写，不写账号、
    /// 人格名、会话编号。
    pub(crate) fn room_id(&self, room: &Room) -> String {
        self.salt.room_id(room)
    }

    /// 等回合索引的后台把这时交过的都写完（施工 V-9），连同这时在补齐旧会话的（施工 R-2 下、R-28）：搜回合库、删会话以前，
    /// 测试照它再看回合库。不等补齐的，删掉的会话会被补回来，载入时也会当成已经补过、不再补。先等那一条道：回合库是那边
    /// 头一次写的时候才开的，补齐也是那时才起。
    pub fn settle(&self) {
        self.lanes.settle();
        self.backfills.settle();
    }

    /// 接上照意思找的那一路（施工 R-5 下）：核心起来、找好小程序和缓存目录以后接一次；接过的再接不算，交回 `false`。
    pub fn give_vectors(&self, vectors: Arc<Vectors>) -> bool {
        self.vectors.set(vectors).is_ok()
    }

    /// 照意思找的那一路：没接的没有。
    pub fn vectors(&self) -> Option<&Arc<Vectors>> {
        self.vectors.get()
    }

    /// 装上整理会话的端口（施工 D-1 上）。交回装没装上：装过一次的不再换。
    pub fn give_dream_sessions(&self, sessions: Arc<dyn DreamSessions>) -> bool {
        self.dream_sessions.set(sessions).is_ok()
    }

    /// 整理会话的端口：没装的没有。
    pub fn dream_sessions(&self) -> Option<&Arc<dyn DreamSessions>> {
        self.dream_sessions.get()
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

/// 接上会话 `session` 的记忆：回合索引（交给后台接上、往后照它写，施工 V-9，`memory/lane.rs`）和三件工具的端口
/// （[`Calls`]）。范围是 `off` 的（子会话、`--no-memory`）不接，核心没交记忆的（测试里自己造的）没有。在阻塞线程里调。
///
/// 记忆放在哪照范围（`memory.md`「范围」）：跟着人格的在记忆账号 `account` 的那一间，只在这个会话里的在会话自己的目录
/// 里（属主 `owner` 的）。听众照属主，不照记忆账号：出厂人格的记忆归属主，系统账号开的会话归管理员，听的人还是属主。
///
/// 跟着人格的、中途换过人格的（施工 P-13，`recast.rs`）：换过来以前的几轮当底子，抽取从换的那一条以后抽。
///
/// 场所会话的回合也进回合索引，回合库记下它在哪个场所（施工 R-13 下，`memory.md` 第一条第 1 款）；哪儿都搜得到全部（施工
/// R-29，第九条）。原来（施工 R-2 再补）场所会话不进、载入时拿掉，回合库的条目那时还没有场所。
/// 回合索引照 `laid` 铺、照 `source` 读回日志：底子（分叉出来的支线才有，施工 R-13 三补）接在会话自己的前面。
/// 那一间里她叫什么照 `personas` 里这个人格的名字记下（施工 E-1 修，`her.rs`）。`main` 是这个会话是不是主会话（不带 `parent`）：
/// 回合库连同属主记下它从哪来（施工 E-6），搜到以前的对话时照它判她打不打得开。
#[expect(
    clippy::too_many_arguments,
    reason = "接上记忆要的几样各是一样，拼成一个结构体只为过这一条"
)]
pub(crate) fn connect(
    memory: Option<&Arc<Memory>>,
    scope: MemoryScope,
    account: &AccountId,
    owner: &AccountId,
    (persona, personas): (&str, &Personas),
    (session, main): (&SessionId, bool),
    venue: &VenueId,
    (laid, source): (Laid<'_>, Source),
) -> (Option<TurnLane>, Option<Calls>) {
    let room = match scope {
        MemoryScope::Persona => Room::persona(account, persona),
        MemoryScope::Session => Room::session(owner, session),
        MemoryScope::Off => return (None, None),
    };
    let Some(memory) = memory else {
        return (None, None);
    };
    let found = (persona != NO_PERSONA)
        .then(|| personas.find(persona).ok())
        .flatten();
    memory.name_her(
        &room,
        found.as_ref().and_then(|found| found.file.name.as_ref()),
    );
    let floor = match scope {
        MemoryScope::Persona => recast::floor(laid.own(), persona),
        _ => None,
    };
    // 载入的、换人格时读回的：她最近在场所里叫什么、这个群叫什么照日志接上（施工 E-1 修三、E-4 上）。
    let here = keeper::Heard::last(laid.own());
    let lane = memory.lanes.open(lane::Opening {
        recall: Arc::clone(&memory.turns),
        room: room.clone(),
        session: session.clone(),
        origin: Origin {
            owner: owner.to_string(),
            main,
        },
        venue: venue.clone(),
        floor,
        laid: laid.sent(),
        source,
    });
    let listener = Listener::of(account, owner, venue);
    let calls = Calls::new(memory, room, listener, owner, session).since(floor);
    calls.extracting().0.called(here);
    (lane, Some(calls))
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

/// 会话是不是派出去的子代理：有父会话、不是分叉出来的支线（施工 R-13 三补）。分叉出来的支线（群里第二个人 @ 她时分的，C-8）
/// 照父会话的记忆范围：群里她也照样记、照样想起（2026-10-11 项目主人定），支线也是。fork 子代理（施工 A-F 中）也是分叉
/// 出来的，可它是派出去的子代理：照子代理不召回、不记。
pub(crate) fn subagent(created: &SessionCreated) -> bool {
    created.parent.is_some() && (created.forked.is_none() || fork_child(created))
}

/// 会话是不是 fork 子代理（施工 A-F 中）：分叉点在父会话的一轮当中（`forked.within`）。
pub(crate) fn fork_child(created: &SessionCreated) -> bool {
    created
        .forked
        .as_ref()
        .is_some_and(|forked| forked.within.is_some())
}

/// 回合库这一回第一次开：新建的记 `INFO`，重建的、用不了的记 `WARN`（照会话列表的索引的说法）。哪一间写种类和短号
/// `room_id`（施工 LG-1 下：不写账号、人格名、会话编号；R-2 下：开库的四处都从登记过，原来只有会话写回合那一处记）。
fn log_opened(room: &Room, room_id: &str, opened: &Opened) {
    let room_kind = room.kind();
    match opened {
        Opened::Kept => {}
        Opened::Created => {
            tracing::info!(target: TARGET, room_kind, room_id, "memory index created")
        }
        Opened::Rebuilt(why) => {
            tracing::warn!(target: TARGET, room_kind, room_id, reason = %why, "memory index rebuilt");
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, room_kind, room_id, error = %error, "memory index unusable");
        }
    }
}
