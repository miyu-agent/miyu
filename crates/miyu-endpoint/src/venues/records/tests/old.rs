//! 施工 V-39 上以前判官的读法，原样抄在这里当准绳（只是钉下的字照 `texts` 取，同端点的 `texts`）：整份读、整份交给留着
//! 一切的有效历史、照有效历史挑（`records_in`，它和以前一行行全渲染再挑一样，`miyu-assemble` 的对照测试守着）。

use miyu_assemble::group::{Records, Window};
use miyu_assemble::{ForwardTexts, GroupChat};
use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_store::log::OpenError;

use crate::refusal::Refusal;

/// 以前的：`events` 是 `read_events` 读出来的。
pub(super) fn judge(
    events: Result<Vec<Event>, OpenError>,
    msg: Seq,
    window: Window,
    texts: impl FnOnce(Option<&Event>) -> Result<(GroupChat, Option<ForwardTexts>), Refusal>,
) -> Result<Records, Refusal> {
    let events = match events {
        Ok(events) => events,
        Err(OpenError::Missing(_)) => return Err(Refusal::NOT_FOUND),
        Err(_) => return Err(Refusal::BROKEN),
    };
    let (texts, forward) = texts(events.first())?;
    let mut history = History::whole();
    for event in events {
        history.append(event);
    }
    miyu_assemble::group::records_in(&history, msg, window, &texts, forward.as_ref())
        .ok_or(Refusal::BAD_PARAMS)
}
