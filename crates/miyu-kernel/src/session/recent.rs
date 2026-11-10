//! 最近接受的命令编号（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」，不变量 9）。
//!
//! 同一个编号再来，照上一次回应，不再产生事件。只记最近的 [`CAPACITY`] 个，不随日志变长：
//! 断线重发发生在几秒、几分钟之内，再早的编号再来，当新命令。
//!
//! 每个编号另记一份命令的指纹（施工 YC-7）：同一个编号换了参数再来，拒绝，`id_reused`，不悄悄交回上一次的结果。指纹只在
//! 内存里：载入时从日志认回来的编号没有指纹，再来照旧去重（核心重启以后照老规矩）。

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, VecDeque};
use std::hash::{Hash, Hasher};

use super::input::Command;
use crate::id::{CommandId, Seq};

/// 最多记几个编号。
pub(crate) const CAPACITY: usize = 1024;

/// 最近接受的命令编号，和它们产生的事件的序号。
#[derive(Debug, Clone, Default)]
pub(crate) struct Recent {
    /// 编号到它产生的事件的序号。
    events: BTreeMap<CommandId, Vec<Seq>>,
    /// 编号接受的先后，最早的在前，满了就从这头丢。
    order: VecDeque<CommandId>,
    /// 编号到它那个命令的指纹（[`digest`]）。载入时认回来的、不是当场收下的没有。
    digests: BTreeMap<CommandId, u64>,
    /// 正在判的那个命令的编号和指纹（[`Recent::expect`]）：收下时 [`Recent::insert`] 照它记指纹。
    incoming: Option<(CommandId, u64)>,
}

impl Recent {
    /// 这个编号最近接受过的话，它产生的事件的序号。
    pub(crate) fn get(&self, id: &CommandId) -> Option<&[Seq]> {
        self.events.get(id).map(Vec::as_slice)
    }

    /// 要判一个新命令了：记下它的编号和指纹，收下时（[`Recent::insert`] 同一个编号）跟着记。每个新命令都来一次，后来的
    /// 换掉先前的：拒了的不记编号，留着的这一份等下一个命令换掉。
    pub(crate) fn expect(&mut self, id: &CommandId, digest: u64) {
        self.incoming = Some((id.clone(), digest));
    }

    /// 这个编号记着指纹，又和 `digest` 不一样：同一个编号换了参数。没有指纹的（载入时认回来的）不算。
    pub(crate) fn reused(&self, id: &CommandId, digest: u64) -> bool {
        self.digests.get(id).is_some_and(|&seen| seen != digest)
    }

    /// 记下一个接受了的命令。正在判的就是它的，连指纹一起记。满了，丢掉最早的那个。
    ///
    /// 不是当场收下的（读回日志以后才做完的撤销这一类）：中间来过别的命令，正在判的换成了别的编号，就不记指纹，照旧去重。
    pub(crate) fn insert(&mut self, id: CommandId, events: Vec<Seq>) {
        if let Some((_, digest)) = self.incoming.take_if(|(expected, _)| *expected == id) {
            self.digests.insert(id.clone(), digest);
        }
        if self.events.insert(id.clone(), events).is_none() {
            self.order.push_back(id);
        }
        while self.order.len() > CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.events.remove(&oldest);
                self.digests.remove(&oldest);
            }
        }
    }
}

/// 命令的指纹：照它的 `Debug` 写法算的哈希，一样的参数一样，换了参数的（几乎一定）不一样。只在这个进程里比，不落盘，
/// 所以用标准库的哈希就够，不管跨版本稳不稳。
pub(crate) fn digest(command: &Command) -> u64 {
    let mut hasher = DefaultHasher::new();
    format!("{command:?}").hash(&mut hasher);
    hasher.finish()
}
