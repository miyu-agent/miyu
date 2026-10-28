//! 账号的拒绝（施工 Y-1 上；施工 SU-1 起只剩一种）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 这个账号不能调这个方法：包自己的账号改不了全机的东西。
    pub(crate) const NOT_PERMITTED: Refusal = Refusal {
        code: REFUSED,
        reason: "not_permitted",
        data: None,
    };
}
