//! 知识库的几种拒绝（施工 K-1 上，`knowledge.md`「对外的样子」）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 没有这个知识库（施工 K-1 上）：账号下没有这个编号的目录。编号不合写法的是 `bad_params`。
    pub(crate) const UNKNOWN_KB: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_kb",
        data: None,
    };
    /// 库里没有这个文档（施工 K-1 上）：不是文档的、不在库里的、路径跑出去的都算。
    pub(crate) const UNKNOWN_DOCUMENT: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_document",
        data: None,
    };
}
