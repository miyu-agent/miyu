//! 装、卸、升级软件包（连带批扩展的能力）只有替主人干活的连接、属主是主人的会话（施工 Y-5 下下，SU-2 上）：方法、端口进来先过
//! 这一道，装的时候（`manage::clear_of_conflicts`）再兜一次底。

use crate::refusal::Refusal;

/// 这一刻是不是替主人干活：不是的 `not_permitted`。
pub(super) fn may_manage(for_owner: bool) -> Result<(), Refusal> {
    match for_owner {
        true => Ok(()),
        false => Err(Refusal::NOT_PERMITTED),
    }
}

#[cfg(test)]
mod tests;
