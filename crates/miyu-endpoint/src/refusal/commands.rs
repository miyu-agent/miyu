//! 斜杠命令的几种拒绝（施工 O-6、9-7 下、X-14 下，`protocol.md` 的 `command.run`）：`refusal.rs` 到了 500 行，挪到这里。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 斜杠命令认不出（施工 O-6，`command.run`）。
    pub(crate) const UNKNOWN_COMMAND: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_command",
        data: None,
    };
    /// 这个人不能用斜杠命令（施工 O-6）：群里既不是主人、也不是管理的人。
    pub(crate) const COMMAND_NOT_ALLOWED: Refusal = Refusal {
        code: REFUSED,
        reason: "command_not_allowed",
        data: None,
    };
    /// 扩展声明的斜杠命令（施工 X-14 下）：扩展没连上、断了、到时没答。
    pub(crate) const COMMAND_UNAVAILABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "command_unavailable",
        data: None,
    };
    /// 扩展声明的斜杠命令（施工 X-14 下）：扩展答了错、答的写法不对，`data.why` 是它的原话。
    pub(crate) const COMMAND_FAILED: Refusal = Refusal {
        code: REFUSED,
        reason: "command_failed",
        data: None,
    };
    /// 这个命令只有主人本人能用（施工 9-7 下，`/workspace`）：管理的人也不行。
    pub(crate) const OWNER_ONLY: Refusal = Refusal {
        code: REFUSED,
        reason: "owner_only",
        data: None,
    };
}
