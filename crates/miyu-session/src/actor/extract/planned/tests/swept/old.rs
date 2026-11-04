//! 施工 V-43 以前接不上时的读法和拼法，照抄（`memory/extract/prefix.rs` 的 `read` 整份读那一截、`actor/extract.rs` 不等闲的
//! 那一次读回来以后的 `ended_only`、`actor/extract/planned.rs` 的 `planned`），当准绳：整份读成事件，整份拼一份留着一切的
//! 有效历史。

use std::path::Path;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_store::log::{Mark, read_marked};

use super::super::super::Planning;
use crate::memory::{Plan, Voices, plan, remembered_since};

/// 以前读回来的：事件，和要接上时认出的末尾那一处（照 `Debug` 写出来的认出来的几样，比的时候比字）。
pub(super) struct Read {
    pub(super) events: Vec<Event>,
    pub(super) seed: Option<(Mark, String)>,
}

/// 以前的整份读：`seed` 的顺手认出末尾那一处（`Prefix::of` 整份折），`ended` 的读回来以后只留到最后一条 `turn.ended`。
pub(super) fn read(dir: &Path, seed: bool, ended: bool) -> Result<Read, String> {
    let mut events = Vec::new();
    let end = read_marked(dir, None, |segment| {
        if events.is_empty() {
            events = segment;
        } else {
            events.extend(segment);
        }
        true
    })
    .map_err(|error| error.to_string())?;
    let seed = end
        .filter(|_| seed)
        .map(|mark| (mark, super::prefix_of(&events)));
    if ended {
        let cut = events
            .iter()
            .rposition(|event| matches!(event.body, Body::TurnEnded(_)))
            .map_or(0, |at| at + 1);
        events.truncate(cut);
    }
    Ok(Read { events, seed })
}

/// 以前整份读回来以后的拼法。
pub(super) fn planned(events: Vec<Event>, after: Seq, how: &Planning<'_>) -> Plan {
    let mut history = History::whole().owned_by(how.own.clone());
    for event in events {
        history.append(event);
    }
    let spoken = how.assembler.spoken(&history, after);
    let remembered = remembered_since(&history, after);
    let mut voices = Voices::continued(None, history.events(), how.budget);
    voices.limit = how.limit;
    plan(
        &spoken,
        &remembered,
        how.min_turns,
        how.offset,
        how.texts,
        &voices,
        how.name,
    )
}
