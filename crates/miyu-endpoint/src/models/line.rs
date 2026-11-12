//! 会话钉的压缩触发线（施工 CT-1，`docs/blueprint/compaction.md` 第二条第 2 条）：`session.configure`、`session.create` 的
//! `compaction_line` 共用的查法。
//!
//! 窗口的百分之几，整数，范围照配置项 `compaction.line` 声明的（`CompactionSettings::line_range`，不另抄一份）；不在范围里的
//! `bad_params`，`data.field` 是 `compaction_line`，什么都不记。`null` 是去掉、回到跟着 `compaction.line`。下一个回合开始时生效。

use miyu_kernel::session::Command;
use miyu_session::settings::CompactionSettings;

use crate::refusal::Refusal;

/// 要交给内核的命令：`percent` 外层没有的是不换（没有命令），里面没有的是去掉。
///
/// # Errors
///
/// 不在 `compaction.line` 的范围里：`bad_params`，`data.field` 是 `compaction_line`。
pub(crate) fn command(percent: Option<Option<u64>>) -> Result<Option<Command>, Refusal> {
    let Some(percent) = percent else {
        return Ok(None);
    };
    if percent.is_some_and(|percent| !CompactionSettings::line_range().contains(&percent)) {
        return Err(Refusal::bad_field("compaction_line"));
    }
    Ok(Some(Command::SetCompactionLine { percent }))
}
