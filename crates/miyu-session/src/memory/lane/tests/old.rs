//! 原来的接法（施工 V-9 起、V-37 下以前 `lane.rs` 的 `connect`、`Open::advance`、`Source::events`），照抄当准绳：自己的、
//! 底子各整份读出来，照换人格那一条切开，交给整份铺的 [`Turns::connect`]；恢复了、不记得字的那几轮整份读回再算。只把「记
//! 一行」改成交回读不了的原因，好和新的比报的是哪一个错。

use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, TurnId};
use miyu_recall::TurnItem;
use miyu_store::log::{OpenError, read_events};

use super::super::super::Turns;
use super::super::super::recast;
use super::super::{Laid, Open, Opening, Source};

/// 原来的 `connect`。
pub(super) fn connect(opening: Opening) -> Result<Open, OpenError> {
    let Opening {
        recall,
        room,
        session,
        origin,
        venue,
        floor,
        laid,
        source,
    } = opening;
    let own = match laid {
        Laid::Empty => Vec::new(),
        Laid::Given(own) => own,
        Laid::Read(_) => read_events(&source.dir)?,
    };
    let base = match source
        .base
        .as_ref()
        .map(|base| base.events_from(Seq::FIRST))
    {
        None => Vec::new(),
        Some(base) => base?,
    };
    let (base, mine) = recast::split(&base, &own, floor);
    let turns = Turns::connect(&recall, &room, (&session, origin), &venue, &base, mine)
        .expect("总是交回一个");
    let upto = own.last().map(|event| event.seq);
    Ok(Open {
        turns,
        upto,
        source,
    })
}

/// 原来的 `Open::advance`。
pub(super) fn advance(open: &mut Open, mut events: Vec<Event>) {
    if let Some(upto) = open.upto {
        events.retain(|event| event.seq > upto);
    }
    let Some(last) = events.last() else {
        return;
    };
    open.upto = Some(last.seq);
    let source = &open.source;
    open.turns.advance(&events, |lost| lost_turns(source, lost));
}

/// 原来写库遇到 `Change::Lost`：`Source::events` 整份读回，`replay` 以后挑那几轮。
fn lost_turns(source: &Source, lost: &[TurnId]) -> Result<Vec<TurnItem>, String> {
    let events = events(source)?;
    Ok(miyu_recall::replay(&events)
        .into_iter()
        .filter(|item| lost.contains(&item.turn))
        .collect())
}

/// 原来的 `Source::events`：底子接上会话自己的整份日志。
fn events(source: &Source) -> Result<Vec<Event>, String> {
    let mut events = match &source.base {
        Some(base) => base
            .events_from(Seq::FIRST)
            .map_err(|error| error.to_string())?,
        None => Vec::new(),
    };
    events.extend(read_events(&source.dir).map_err(|error| error.to_string())?);
    Ok(events)
}
