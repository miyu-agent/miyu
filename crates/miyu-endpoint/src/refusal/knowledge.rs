//! 知识库的几种拒绝（施工 K-1 上，`knowledge.md`「对外的样子」）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 没有这个知识库（施工 K-1 上）：没有这个编号的目录。编号不合写法的是 `bad_params`。
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
    /// 要建、要挂的库已经有了（施工 K-7 上）。
    pub(crate) const KB_EXISTS: Refusal = Refusal {
        code: REFUSED,
        reason: "kb_exists",
        data: None,
    };
    /// 要加的文档已经有了，没说换（施工 K-7 上）。
    pub(crate) const DOCUMENT_EXISTS: Refusal = Refusal {
        code: REFUSED,
        reason: "document_exists",
        data: None,
    };
    /// 往挂进来的库里加、删（施工 K-7 上，19 N7：只读）。
    pub(crate) const KB_READ_ONLY: Refusal = Refusal {
        code: REFUSED,
        reason: "kb_read_only",
        data: None,
    };

    /// 改、删软件包带来的库（施工 K-8：名字、说明、文档照包，只有自动想起的开关人能改）。
    pub(crate) const KB_PACKAGED: Refusal = Refusal {
        code: REFUSED,
        reason: "kb_packaged",
        data: None,
    };

    /// 要挂的目录用不了（施工 K-7 上）：`data.message` 是英文的一句为什么。
    pub(crate) fn folder_unavailable(why: String) -> Refusal {
        Refusal::with("folder_unavailable", "message", serde_json::json!(why))
    }

    /// 挪不进回收站（施工 K-7 上）：`data.message` 是英文的一句为什么。
    pub(crate) fn trash_failed(why: String) -> Refusal {
        Refusal::with("trash_failed", "message", serde_json::json!(why))
    }
}
