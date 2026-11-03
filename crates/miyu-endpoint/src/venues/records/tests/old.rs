//! 施工 V-39 上以前判官的读法，原样抄在这里当准绳（只是钉下的字照 `texts` 取，同端点的 `texts`）：整份读、整份交给留着
//! 一切的有效历史、照有效历史挑（`records_in`，它和以前一行行全渲染再挑一样，`miyu-assemble` 的对照测试守着）。
//!
//! 施工 V-39 上补以前（V-39 上）会话目录的读法（[`Full`]）也照抄在这里：每一行全解（`read_range`、`read_events`）。判官边读
//! 边记的那一套（`read.rs`、`pass.rs`、`bones.rs`）V-39 上补没动，喂它全解的就是 V-39 上的判官。

use miyu_assemble::group::{Records, Window};
use miyu_assemble::{ForwardTexts, GroupChat};
use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_store::log::{Flow, OpenError, read_events, read_range};

use crate::refusal::Refusal;
use crate::venues::records::read::Log;

/// V-39 上的会话目录：每一行全解。
pub(super) struct Full(pub(super) std::path::PathBuf);

impl Log for Full {
    fn range(
        &mut self,
        from: Seq,
        upto: Option<Seq>,
        each: &mut dyn FnMut(Vec<Event>) -> Flow,
    ) -> Result<(), OpenError> {
        read_range(&self.0, from, upto, each)
    }

    fn whole(&mut self) -> Result<Vec<Event>, OpenError> {
        read_events(&self.0)
    }
}

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
