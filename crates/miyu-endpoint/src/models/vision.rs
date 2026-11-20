//! 会话钉的看图模型（施工 VM-1，`internal/blueprint/models.md`「协议」、「怎么走」第十三条第 4 条）：`session.configure`、
//! `session.create` 的 `vision` 共用的查法和发法。
//!
//! 写法、解析同 `model`（[`super::record_or_none`]：照这时不算项目配置的最终值，解析不出的 `unknown_model`），`null` 是去掉、
//! 回到跟着 `models.vision`。写的是一个空的固定池的当成没写（施工 SIDE-2，同 `model`，设计 43 第三节第 3 条）：
//! `session.configure` 的同写 `null`，`session.create` 的照没写这一格（[`created`]）。`session.create` 带的造好以后发
//! （`pin.rs`）。

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
    let vision = text
        .map(|text| super::record_or_none(core, text))
        .transpose()?
        .flatten();
    Ok(Some(Command::SetVision { vision }))
}

/// `session.create` 的 `vision`：写了的同 [`command`]；没写的、写的是空的固定池的没有命令，造好以后什么都不发。
///
/// # Errors
///
/// 同 [`command`]。
pub(crate) fn created(core: &Core, text: Option<&str>) -> Result<Option<Command>, Refusal> {
    let command = command(core, text.map(Some))?;
    Ok(command.filter(|command| !matches!(command, Command::SetVision { vision: None })))
}
