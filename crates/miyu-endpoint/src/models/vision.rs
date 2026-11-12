//! 会话钉的看图模型（施工 VM-1，`docs/blueprint/models.md`「协议」、「怎么走」第十三条第 4 条）：`session.configure`、
//! `session.create` 的 `vision` 共用的查法和发法。
//!
//! 写法、解析同 `model`（[`super::record`]：照这时不算项目配置的最终值，解析不出的 `unknown_model`），`null` 是去掉、回到
//! 跟着 `models.vision`。`session.create` 带的造好以后发（`pin.rs`）。

use miyu_kernel::session::Command;

use crate::Core;
use crate::refusal::Refusal;

/// 要交给内核的命令：`text` 外层没有的是不换（没有命令），里面没有的是去掉；钉成的引用先照这时的配置解析好。
///
/// # Errors
///
/// 解析不出：`unknown_model`，原话记一行 `DEBUG unknown model`。
pub(crate) fn command(core: &Core, text: Option<Option<&str>>) -> Result<Option<Command>, Refusal> {
    let Some(text) = text else {
        return Ok(None);
    };
    let vision = text.map(|text| super::record(core, text)).transpose()?;
    Ok(Some(Command::SetVision { vision }))
}
