//! 同一个命令编号换了参数再来的拒绝（施工 YC-7，`docs/blueprint/protocol.md`「同一个命令编号」）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 造会话的命令编号重发，除了 `cwd`、`dirs` 别的参数换了：不交回上一次造的那一个。会话里的命令同一个码，内核拒
    /// （`Reason::IdReused`）。
    pub(crate) const ID_REUSED: Refusal = Refusal {
        code: REFUSED,
        reason: "id_reused",
        data: None,
    };
}
