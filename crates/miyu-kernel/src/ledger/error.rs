//! 账本拦下的一条（施工 C-8 上从 `ledger.rs` 挪出来：那一份到了 500 行）。

use std::fmt;

use crate::id::Seq;

/// 一条事件违反了追加的规矩：它是第几条，违反了哪一条。
///
/// 报错是中文，给查问题的人看。违反规矩只会是内核自己的 bug，或者坏了的日志文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerError {
    /// 那条事件写着的序号。
    pub seq: Seq,
    /// 违反了哪一条规矩。
    pub why: String,
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "event {} cannot be appended: {}", self.seq, self.why)
    }
}

impl std::error::Error for LedgerError {}
