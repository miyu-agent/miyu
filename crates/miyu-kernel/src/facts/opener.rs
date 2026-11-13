//! 开这一轮的那条消息是谁发的（施工 LF-6，`docs/blueprint/kernel/session.md` 的 `RunTurnStartHooks`）：回合开始交给挂接点，
//! 记忆照它认出人生循环叫醒的一轮（`memory.md` 第三条第 9 款）。

use crate::event::Body;
use crate::history::History;
use crate::id::TurnId;
use crate::origin::By;

/// 开 `turn` 这一轮的那条消息是谁发的（施工 LF-6，`memory.md` 第三条第 9 款）：回合开始交给挂接点（`Action::RunTurnStartHooks`
/// 的 `opener`），记忆照它认出人生循环叫醒的一轮。照这一轮 `turn.started` 的 `trigger` 在有效历史里找那条 `message.user`，
/// 原样交它的 `by`，什么意思由挂接点自己判（内核不替模块定哪种来源算什么）。没有 `trigger` 的（人要的压缩单开的）、触发的
/// 不是消息的（回报），没有。
pub fn opener(history: &History, turn: TurnId) -> Option<By> {
    // 同 `said`：这一轮的 `turn.started` 和触发它的那条都在末尾附近，从后往前找。
    let events = history.events();
    let trigger = events.iter().rev().find_map(|event| match &event.body {
        Body::TurnStarted(started) if event.seq == turn.started() => started.trigger,
        _ => None,
    })?;
    events.iter().rev().find_map(|event| match &event.body {
        Body::MessageUser(_) if event.seq == trigger => Some(event.by.clone()),
        _ => None,
    })
}
