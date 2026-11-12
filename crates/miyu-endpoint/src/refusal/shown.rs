//! `session.shown` 的拒绝（施工 SW-1，`docs/blueprint/protocol.md` 的 `session.shown`）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 没有这次展示：会话没有、这个连接读不到、日志里没有这次调用的结果、结果里没有 `file.shown`。几种不分开说（同
    /// `unknown_blob`），免得拿它探会话编号。
    pub(crate) const UNKNOWN_SHOWN: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_shown",
        data: None,
    };
    /// 展示的文件大小或修改时间和记的对不上：改过、换过。
    pub(crate) const FILE_CHANGED: Refusal = Refusal {
        code: REFUSED,
        reason: "file_changed",
        data: None,
    };
    /// 展示的文件没了，或者不是普通文件了。
    pub(crate) const FILE_GONE: Refusal = Refusal {
        code: REFUSED,
        reason: "file_gone",
        data: None,
    };
}
