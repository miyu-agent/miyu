//! 凭据决定的闸（施工 W-8、Y-1 上）：用一次性码连上的只能设密码，用邀请码连上的只能注册；不是管理员的人（成员）在 Y-2 以前
//! 只能管自己的账号，别的方法一律 `not_permitted`：端点里还有几十处照 `core.admin` 干活，成员调了就等于以管理员的身份干活。
//! Y-2 把它们改成照连接的账号以后，成员那一道拿掉。还没有机主的设置那一段（施工 Y-12 中上），本机令牌的连接只能握手、注册机主。
//! 成员照名单放（施工 Y-2 下下上，[`members`]）：名单里的都只碰得到他自己的。管理员组里的人（施工 Y-12 中下）：改系统那一层
//! 的要进管理员模式，平时多读得到这台电脑的几样（施工 Y-12 下，[`system`]）。

use miyu_kernel::id::AccountId;
use serde_json::Value;

use crate::Core;
use crate::login::Via;
use crate::login::admins::is_admin;
use crate::refusal::Refusal;

mod members;
mod system;

/// 这一个请求的样子：方法、参数、带没带 `"scope": "system"`，连接握手时声明没声明认识管理员模式（施工 Y-12 下）。
pub(super) struct Asked<'a> {
    pub(super) method: &'a str,
    pub(super) params: &'a Value,
    pub(super) system: bool,
    pub(super) scoped: bool,
}

/// 用凭据 `via` 连上、现在是 `account` 的连接能不能这样请求：不能的交回拒绝。
pub(super) fn refuse(
    core: &Core,
    via: Option<&Via>,
    account: &AccountId,
    asked: &Asked<'_>,
) -> Option<Refusal> {
    let (method, params, system, scoped) = (asked.method, asked.params, asked.system, asked.scoped);
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
        // 核心拉起的扩展照它的能力挡（`rights.rs`）；管理员模式只给人（施工 Y-12 下）。
        Some(Via::Spawned) => system.then_some(Refusal::NOT_PERMITTED),
        // 还没握手的：握手照常，带的 `scope` 不看（头在管理员模式里重连，第一条也带着）。
        None => None,
        Some(_) if is_admin(core, account) => {
            let touches = system::touches_system(method, params);
            if scoped && touches && !system {
                return Some(Refusal::ADMIN_MODE_REQUIRED);
            }
            // 机主什么都调得了（扩展那一侧的方法也是）；组里别的人照成员的名单，加上读这台电脑的、改系统那一层的。
            let allowed = *account == core.admin
                || touches
                || system::reads(method, params)
                || members::allowed(method, params);
            (!allowed).then_some(Refusal::NOT_PERMITTED)
        }
        // 不是管理员的人进不了管理员模式。
        Some(_) if system => Some(Refusal::NOT_PERMITTED),
        Some(_) => (!members::allowed(method, params)).then_some(Refusal::NOT_PERMITTED),
    }
}

#[cfg(test)]
mod tests;
