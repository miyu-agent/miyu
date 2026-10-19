//! 中途换了人格的会话（施工 P-13，`docs/designs/16-人格与预设.md` Y5）：跟着人格的记忆从换的那一条起归新人格那一间，以前的
//! 几轮留在原来那一间、不搬。回合索引把那以前的当底子（只为认得还没回的人话，不在新那一间记），抽取从那以后抽。

use std::borrow::Cow;
use std::sync::{Arc, RwLock};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;
use miyu_store::recall::{NO_PERSONA, Room};

/// 会话的记忆现在放在哪一间（施工 P-13）：[`crate::Handle`] 和会话的 actor 共用一份，换人格时 actor 改它，协议照它找。
pub(crate) type RoomCell = Arc<RwLock<Option<Room>>>;

/// 造会话、载入时的那一格：记忆的端口放在哪一间，没接记忆的没有。
pub(crate) fn room_cell(calls: Option<&super::Calls>) -> RoomCell {
    Arc::new(RwLock::new(calls.map(|calls| calls.room().clone())))
}

/// 会话日志 `events` 里最近一条换成人格 `persona`（无人格的是 [`NO_PERSONA`]）的 `policy.changed`：这一间的记忆从它以后
/// 算。没换成过它的（一直是它的、换成别的还没生效的）没有。
pub(crate) fn floor(events: &[Event], persona: &str) -> Option<Seq> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::PolicyChanged(changed) => changed
            .persona
            .as_deref()
            .filter(|to| named(to) == persona)
            .map(|_| event.seq),
        _ => None,
    })
}

/// 照 `floor` 把底子 `base` 和自己的事件 `events` 分开：到它为止的（连同它）并进底子。没有的照旧。
pub(crate) fn split<'a>(
    base: &'a [Event],
    events: &'a [Event],
    floor: Option<Seq>,
) -> (Cow<'a, [Event]>, &'a [Event]) {
    let Some(floor) = floor else {
        return (Cow::Borrowed(base), events);
    };
    let at = events.partition_point(|event| event.seq <= floor);
    let mut joined = base.to_vec();
    joined.extend_from_slice(&events[..at]);
    (Cow::Owned(joined), &events[at..])
}

/// `policy.changed` 里写的人格：空的是换成无人格。
fn named(persona: &str) -> &str {
    if persona.is_empty() {
        NO_PERSONA
    } else {
        persona
    }
}

#[cfg(test)]
mod tests;
