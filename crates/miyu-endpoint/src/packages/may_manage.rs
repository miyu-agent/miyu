//! 成员不能改机器上的软件包（施工 Y-5 下下，设计 06 U19 第 5 条、U23）：这一版只有管理员能装、升级、卸、装回出厂的包，启用扩展
//! （连带批它的能力）。带进程的包以它自己的账号跑、替人干活，只该管理员装；不带进程的（技能、脚本、主题、人格、预设、表情包）U23
//! 要成员装进他自己的家目录、只对自己生效，现在装的地方是机主家目录那一层、对所有人生效，所以也先不给（主会话 2026-10-24 定），
//! 等「装进自己家目录」那一步再放开。协议那条（`manage::clear_of_conflicts`）和会话里 `package` 工具的端口（`port.rs` 的
//! `managing`）都照它判。

use crate::refusal::Refusal;

/// 这一刻是不是管理员改机器上的软件包：不是的 `not_permitted`。
pub(super) fn may_manage(admin: bool) -> Result<(), Refusal> {
    match admin {
        true => Ok(()),
        false => Err(Refusal::NOT_PERMITTED),
    }
}

#[cfg(test)]
mod tests;
