//! 排好、不重的一串序号（施工 LW-37）：账本记旁听、记当过触发的用它，不用 `BTreeSet`。
//!
//! 大群的会话载入时，旁听的序号一条条记下来，`BTreeSet` 每五六条就要分配一个小节点，和载入的事件穿插着分配；事件放掉以后，
//! 这些散着的小节点把它们所在的内存页钉住，还不回系统。连续的一块就没有这个问题。
//!
//! 记的几乎都是比最后一个大的（序号照先后分配），直接接在后面；不是的，二分找到位置插进去，挪动它后面的那几个。查用二分。
//! 调试输出照 `BTreeSet` 的写法（`{Seq(2), Seq(3)}`），账本的调试输出和改之前逐字节相同。

use std::fmt;

use crate::id::Seq;

/// 从小到大排好、不重的序号。
#[derive(Clone, Default, PartialEq, Eq)]
pub(in crate::ledger) struct Seqs(Vec<Seq>);

impl Seqs {
    /// 有没有 `seq`。
    pub(in crate::ledger) fn contains(&self, seq: Seq) -> bool {
        self.0.binary_search(&seq).is_ok()
    }

    /// 记下 `seq`，已经有的不再记。比最后一个大的接在后面；不然二分找位置，挪开后面的插进去，最坏挪动全部 `n` 个。
    pub(in crate::ledger) fn insert(&mut self, seq: Seq) {
        if self.0.last().is_none_or(|last| *last < seq) {
            self.0.push(seq);
        } else if let Err(at) = self.0.binary_search(&seq) {
            self.0.insert(at, seq);
        }
    }

    /// 一个个记下。
    pub(in crate::ledger) fn extend(&mut self, seqs: &[Seq]) {
        for seq in seqs {
            self.insert(*seq);
        }
    }
}

impl fmt::Debug for Seqs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(&self.0).finish()
    }
}

#[cfg(test)]
mod tests;
