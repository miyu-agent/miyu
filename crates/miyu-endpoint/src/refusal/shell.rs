//! 人用 `!` 跑命令的拒绝（施工 SH-1 上，设计 57 第二节第 1、2 条）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 不是人在头里打的：带 `from` 的、扩展替人说的、场所会话，都不能跑命令。
    pub(crate) const SHELL_NOT_ALLOWED: Refusal = Refusal {
        code: REFUSED,
        reason: "shell_not_allowed",
        data: None,
    };
    /// 没有跑命令的工具：装着的包里一件都没有，或者包关了。
    pub(crate) const SHELL_UNAVAILABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "shell_unavailable",
        data: None,
    };
}
