//! 凭据决定的闸（施工 W-8、Y-1 上）：用一次性码连上的只能设密码，用邀请码连上的只能注册；不是管理员的人（成员）在 Y-2 以前
//! 只能管自己的账号，别的方法一律 `not_permitted`：端点里还有几十处照 `core.admin` 干活，成员调了就等于以管理员的身份干活。
//! Y-2 把它们改成照连接的账号以后，成员那一道拿掉。还没有机主的设置那一段（施工 Y-12 中上），本机令牌的连接只能握手、注册机主。

use miyu_kernel::id::AccountId;

use crate::Core;
use crate::login::Via;
use crate::refusal::Refusal;

/// 用凭据 `via` 连上、现在是 `account` 的连接能不能调 `method`：不能的交回拒绝。
pub(super) fn refuse(
    core: &Core,
    via: Option<&Via>,
    account: &AccountId,
    method: &str,
) -> Option<Refusal> {
    match via {
        Some(Via::Code) => (!matches!(method, "hello" | "human.get" | "account.setup"))
            .then_some(Refusal::SETUP_FIRST),
        // 设置那一段（施工 Y-12 中上，还没有机主）：本机令牌的连接只能握手、注册机主、要一次性码给远程的网页设置。
        Some(Via::Token) | None if core.setting_up() => (!matches!(
            method,
            "hello" | "human.get" | "account.setup" | "account.setup_code"
        ))
        .then_some(Refusal::SETUP_FIRST),
        Some(Via::Invite(_)) => (!matches!(method, "hello" | "human.get" | "account.register"))
            .then_some(Refusal::REGISTER_FIRST),
        // 核心拉起的扩展照它的能力挡（`rights.rs`）。
        Some(Via::Spawned) | None => None,
        Some(_) if *account == core.admin => None,
        Some(_) => (!matches!(method, "hello" | "human.get" | "account.logout"))
            .then_some(Refusal::NOT_PERMITTED),
    }
}
