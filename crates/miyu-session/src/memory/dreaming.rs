//! 整理的时候她正在写什么（施工 R-21，`docs/blueprint/memory.md` 第七条第 12 款）：合并那一次边收边读（`miyu_recall::merge::
//! Watch`），写完一条改的、作废的就推一条，摘要推写到第几个字；订阅着这一间记忆日志的头照它实时画「窥探梦境」（2026-10-19
//! 项目主人定）。
//!
//! - 不落盘：同压缩的 `compaction.progress`，只推给这时订阅着的，掉了就掉了；落的照全部核对（几条之间的那几道），可能少几条，
//!   真相是之后记忆日志里那几行。
//! - 一个核心一路广播（[`Dreams`]），带着哪一间；订阅的一头（[`DreamFeed`]）只要自己那一间的、说的那一条这次的听众听得见的
//!   （同 `follow` 的挑法）。读得慢掉了队的跳过掉的那几条，接着收。
//! - 推以前遮过 key（同落的）。

use miyu_kernel::id::Seq;
use miyu_recall::merge::Seen;
use miyu_store::recall::Room;
use tokio::sync::broadcast;

use super::Keeper;

/// 一路广播最多攒几条没读走的：一批写出来的几十条够了，再慢的掉队。
const ROOM: usize = 256;

/// 正在合的那一批（照 `upto` 对上它的 `ext.memory.merging`）她刚写完的一样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dreaming {
    /// 哪一批：同它的 `merging`、`merged` 的 `upto`。
    pub upto: Seq,
    /// 写完的那一样（遮过 key 的）。
    pub seen: Seen,
}

/// 一个核心一路：带着哪一间。
#[derive(Debug)]
pub(crate) struct Dreams {
    sender: broadcast::Sender<(Room, Dreaming)>,
}

impl Default for Dreams {
    fn default() -> Dreams {
        let (sender, _) = broadcast::channel(ROOM);
        Dreams { sender }
    }
}

impl Dreams {
    /// 推一条：没有人订阅的不要紧。
    pub(crate) fn tell(&self, room: &Room, dreaming: Dreaming) {
        // 没有收的一头：没人看。
        let _nobody = self.sender.send((room.clone(), dreaming));
    }
}

/// 订阅着一间的一头收的。
#[derive(Debug)]
pub struct DreamFeed {
    receiver: broadcast::Receiver<(Room, Dreaming)>,
    keeper: Keeper,
}

impl DreamFeed {
    /// 下一条：别的间的、这次的听众听不见的跳过；掉了队的跳过掉的接着收。核心那一头放下了的交回没有。
    pub async fn next(&mut self) -> Option<Dreaming> {
        loop {
            match self.receiver.recv().await {
                Ok((room, dreaming)) => {
                    if room == *self.keeper.room() && self.keeper.hears_dreaming(&dreaming) {
                        return Some(dreaming);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

impl Keeper {
    /// 这一间往后的「她正在写什么」。
    pub(crate) fn dream_feed(&self) -> DreamFeed {
        DreamFeed {
            receiver: self.memory.dreams.sender.subscribe(),
            keeper: self.clone(),
        }
    }

    /// 这次的听众听不听得见：说的那一条照 `follow` 的挑法（底账里没有的、摘要照给）。
    fn hears_dreaming(&self, dreaming: &Dreaming) -> bool {
        let id = match &dreaming.seen {
            Seen::Revised(revised) => revised.id,
            Seen::Retired(retired) => retired.id,
            Seen::Summary { .. } => return true,
        };
        let Ok(log) = self.log() else {
            return false;
        };
        log.book(|book| {
            book.get(id)
                .is_none_or(|entry| !entry.cleared && self.hears(entry))
        })
    }
}
