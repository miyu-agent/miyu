//! 改系统那一层的软件包只有管理员（施工 Y-5 下下，设计 06 U19 第 5 条、U23）：装、升级、装回出厂的包（连带批扩展的能力）。
//! 施工 U23 下上起成员装进自己那一层（`scope.rs`、`mine.rs`），会话里的 `package` 工具也一律装进属主自己那里；这一道只守系统
//! 那一层的装（`manage::clear_of_conflicts`）：请求进得来这里已经过了管理员模式那一道，它是兜底。

use crate::refusal::Refusal;

/// 这一刻是不是管理员改系统那一层的软件包：不是的 `not_permitted`。
pub(super) fn may_manage(admin: bool) -> Result<(), Refusal> {
    match admin {
        true => Ok(()),
        false => Err(Refusal::NOT_PERMITTED),
    }
}

#[cfg(test)]
mod tests;
