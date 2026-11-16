//! 施工 V-39 上以前的 `records_in`、`quote_of`，原样抄在这里当准绳：从有效历史一行行全渲染再挑（`same.rs` 拿新的比它）。

use std::collections::BTreeMap;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{ExternalId, Seq};

use crate::group::records::{Records, Window};
use crate::group::{content, recall_mark, record, sender, you, you_said};
use crate::texts::{ForwardTexts, GroupChat};

/// 以前的 `records_in`。
pub(super) fn records_in(
    history: &History,
    msg: Seq,
    window: Window,
    texts: &GroupChat,
    forward: Option<&ForwardTexts>,
) -> Option<Records> {
    let events = history.events();
    let at = events.binary_search_by_key(&msg, |event| event.seq).ok()?;
    let judged = &events[at];
    let Body::MessageUser(message) = &judged.body else {
        return None;
    };
    let venue = message.venue.as_ref()?;
    // 撤回的只认要判的那一条以前记下的。
    let recalled: BTreeMap<&str, &ExternalId> = events[..at]
        .iter()
        .filter_map(|event| match &event.body {
            Body::VenueRecalled(recalled) => Some((recalled.msg.as_str(), &recalled.by)),
            _ => None,
        })
        .collect();
    let lines: Vec<String> = events[..at]
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageUser(said) => {
                let venue = said
                    .venue
                    .as_ref()
                    .filter(|venue| !venue.asleep && !venue.backfill)?;
                let mark = recalled
                    .get(venue.msg.as_str())
                    .map(|by| recall_mark(&event.by, by, venue.show_ids))
                    .unwrap_or_default();
                Some(record(
                    event.at,
                    &event.by,
                    venue,
                    &said.blocks,
                    (texts, forward),
                    (&mark, ""),
                ))
            }
            Body::VenueDelivered(delivered) => Some(you(event, delivered, texts)),
            _ => None,
        })
        .collect();
    let mut kept = String::new();
    let shown = &lines[window.start(lines.len())..];
    for line in shown {
        kept.push_str(line);
        kept.push('\n');
    }
    let quote = venue
        .reply_to
        .as_deref()
        .map(|reply_to| quote_of(&events[..at], reply_to, texts))
        .unwrap_or_default();
    Some(Records {
        count: shown.len(),
        records: kept,
        current: record(
            judged.at,
            &judged.by,
            venue,
            &message.blocks,
            (texts, forward),
            ("", &quote),
        ),
    })
}

/// `events` 里最后一条编号是 `reply_to` 的：` 发的人: 内容`，她发的 ` [you]: 内容`；没有的是空的。
fn quote_of(events: &[Event], reply_to: &str, texts: &GroupChat) -> String {
    events
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::MessageUser(said) => said
                .venue
                .as_ref()
                .filter(|venue| venue.msg == reply_to)
                .map(|venue| {
                    format!(
                        " {}: {}",
                        sender(&event.by, venue),
                        content(&said.blocks, &venue.media, texts)
                    )
                }),
            Body::VenueDelivered(delivered) if delivered.msg == reply_to => {
                Some(format!(" [you]: {}", you_said(delivered, texts)))
            }
            _ => None,
        })
        .unwrap_or_default()
}
