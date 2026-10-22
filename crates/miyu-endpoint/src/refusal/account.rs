//! 账号的拒绝（施工 Y-1 上，设计 06 第二版）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 这个账号不能调这个方法：现在是成员的连接只能管自己的账号（Y-2 以前的临时闸），不是管理员的发不了邀请码、管不了账号。
    pub(crate) const NOT_PERMITTED: Refusal = Refusal {
        code: REFUSED,
        reason: "not_permitted",
        data: None,
    };
    /// 用邀请码连上的，先注册。
    pub(crate) const REGISTER_FIRST: Refusal = Refusal {
        code: REFUSED,
        reason: "register_first",
        data: None,
    };
    /// 注册的用户名已经有人用了，或者是保留的名字。
    pub(crate) const USERNAME_TAKEN: Refusal = Refusal {
        code: REFUSED,
        reason: "username_taken",
        data: None,
    };
    /// 没有这个账号，或者它不是能停用、启用的成员（管理员、系统账号）。
    pub(crate) const UNKNOWN_ACCOUNT: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_account",
        data: None,
    };
    /// 没有这个编号的邀请码，或者它已经用掉了。
    pub(crate) const UNKNOWN_INVITE: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_invite",
        data: None,
    };
    /// 握手的邀请码对不上、过期了、用过了、撤掉了。
    pub(crate) const BAD_INVITE: Refusal = Refusal {
        code: REFUSED,
        reason: "bad_invite",
        data: None,
    };
}
