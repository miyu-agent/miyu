//! 判官看的群聊记录（施工 O-24，`docs/blueprint/chat.md` 第六条 `Ask.records`、`Ask.current`）：要判的那一条渲染成一行，它之前的
//! 群里的话一行一条，和她看到的同一个写法（18 第七节「判官和她各看各的」）。
//!
//! 收场所的 `message.user`（旁听的、开过回合的都收，睡着时收到的不收）和 `venue.delivered` 的 `[you]` 行（判官看不到她的回复，
//! 主线的也收）；撤回只认要判的那一条以前记下的。从新往旧取 `count` 条，照日志的先后排。
//!
//! 要判的那一条引用了别的（施工 O-41，照旧版判官看的）：引用的编号后面空一格写那一条的发的人和内容，照日志里那一行的写法
//! （`reply-to: msg=8805 小林: @Miyu 在吗`，她发的写 `[you]`）；认要判的那一条以前日志里最后一条这个编号的，睡着时收到的、
//! 补拉回来的也认。日志里没有的照旧只写编号。之前的记录、她看到的一行照旧只写编号。
//!
//! 挑行和渲染分两半（施工 V-39 上）：[`Picked`] 是挑好的（要判的那一条、之前一共几条、最后几条、撤回、引用的那一条），
//! [`records_from`] 只渲染要显示的那几行。判官的端点边读日志边挑，[`records_in`] 从有效历史挑，渲染是同一份。

use std::collections::BTreeMap;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{ExternalId, Seq};

use super::{content, recall_mark, record, sender, you, you_said};
use crate::texts::{ForwardTexts, GroupChat};

/// 判官看的两样：之前的记录（一行一条，每行以换行结尾，没有的是空的）和要判的那一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Records {
    /// 之前的记录。
    pub records: String,
    /// 要判的那一条。
    pub current: String,
    /// 之前的记录有几条。
    pub count: usize,
}

/// 之前的记录取哪一段（施工 O-44）：最近的 `count` 条；写了步长 `grow` 的，起点照步长对齐（`(n − count) / grow × grow`，
/// 不到 0 的从头），条数在 `count` 到 `count + grow − 1` 之间。起点一段时间不动，判官的请求前缀就稳，缓存命中得上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// 至少几条（前面不够的照有的）。
    pub count: usize,
    /// 起点对齐的步长；没有的就是最近的 `count` 条。
    pub grow: Option<usize>,
}

impl Window {
    /// 最多显示几条（施工 V-39 上）：没有步长的 `count` 条，有步长的 `count + grow − 1` 条。判官的端点照它留最后几条。
    pub fn most(self) -> usize {
        match self.grow {
            Some(grow) if grow > 0 => self.count + grow - 1,
            _ => self.count,
        }
    }

    /// 之前一共 `n` 条里显示几条（施工 V-44：判官的记录窗照它看留着的够不够）。
    pub fn shown(self, n: usize) -> usize {
        n - self.start(n)
    }

    /// 一共 `n` 条里从第几条起。
    fn start(self, n: usize) -> usize {
        let plain = n.saturating_sub(self.count);
        match self.grow {
            Some(grow) if grow > 0 => plain / grow * grow,
            _ => plain,
        }
    }
}

/// 第 `msg` 条之前至多 `count` 条群里的话，和第 `msg` 条那一行；它不是带 `venue` 的 `message.user` 的，没有。合并转发照
/// `forward` 写在那一条下面（施工 O-34，和她看到的一样；没有那几句的字的不写）。
pub fn records(
    history: &History,
    msg: Seq,
    count: usize,
    texts: &GroupChat,
    forward: Option<&ForwardTexts>,
) -> Option<Records> {
    let window = Window { count, grow: None };
    records_in(history, msg, window, texts, forward)
}

/// 同 [`records`]，之前的记录照 `window` 取（施工 O-44）。从有效历史挑出 [`Picked`]，照 [`records_from`] 渲染（施工 V-39 上）。
pub fn records_in(
    history: &History,
    msg: Seq,
    window: Window,
    texts: &GroupChat,
    forward: Option<&ForwardTexts>,
) -> Option<Records> {
    let events = history.events();
    let at = events.binary_search_by_key(&msg, |event| event.seq).ok()?;
    let judged = &events[at];
    let before = &events[..at];
    let recalled: BTreeMap<&str, &ExternalId> = before.iter().filter_map(recall).collect();
    let lines: Vec<&Event> = before.iter().filter(|event| on_record(event)).collect();
    let quote = reply_to(judged).and_then(|reply_to| {
        before
            .iter()
            .rev()
            .find(|event| quoted_as(event) == Some(reply_to))
    });
    let picked = Picked {
        judged,
        before: lines.len(),
        last: &lines,
        recalled: &recalled,
        quote,
    };
    records_from(&picked, window, texts, forward)
}

/// 挑好的、要渲染的几样（施工 V-39 上）：判官的端点边读日志边挑，不攒整份有效历史；[`records_in`] 从有效历史挑。都照「留着
/// 一切的有效历史」（`History::whole`）里要判的那一条以前的算。
#[derive(Debug, Clone, Copy)]
pub struct Picked<'a> {
    /// 要判的那一条。
    pub judged: &'a Event,
    /// 它以前的记录（[`on_record`] 的那些）一共几条。
    pub before: usize,
    /// 它以前的记录里最后的几条，照先后：至少有窗口要显示的那么多（[`Window::most`]），一共不够的是全部。
    pub last: &'a [&'a Event],
    /// 它以前记下的撤回：平台编号 → 谁撤的，同一个编号的照最后一条。
    pub recalled: &'a BTreeMap<&'a str, &'a ExternalId>,
    /// 它引用的那一条：它以前最后一条 [`quoted_as`] 是那个编号的；不引用的、找不到的没有。
    pub quote: Option<&'a Event>,
}

/// 照挑好的渲染（施工 V-39 上）：之前的记录照 `window` 从 `picked.last` 的末尾取，只渲染要显示的那几行；要判的那一条不是带
/// `venue` 的 `message.user` 的，没有。写法同 [`records_in`] 以前一行行全渲染再挑。
pub fn records_from(
    picked: &Picked<'_>,
    window: Window,
    texts: &GroupChat,
    forward: Option<&ForwardTexts>,
) -> Option<Records> {
    let judged = picked.judged;
    let Body::MessageUser(message) = &judged.body else {
        return None;
    };
    let venue = message.venue.as_ref()?;
    let shown = window.shown(picked.before);
    debug_assert!(picked.last.len() >= shown, "挑的比要显示的少");
    let shown = &picked.last[picked.last.len().saturating_sub(shown)..];
    let mut kept = String::new();
    for event in shown {
        if let Some(line) = line(event, picked.recalled, texts, forward) {
            kept.push_str(&line);
            kept.push('\n');
        }
    }
    let quote = venue
        .reply_to
        .as_ref()
        .and(picked.quote)
        .and_then(|quoted| quote_of(quoted, texts))
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

/// 算不算之前的记录里的一行：场所的 `message.user`（睡着时收到的、补拉回来的不算）和 `venue.delivered`（施工 V-39 上）。
pub fn on_record(event: &Event) -> bool {
    match &event.body {
        Body::MessageUser(said) => said
            .venue
            .as_ref()
            .is_some_and(|venue| !venue.asleep && !venue.backfill),
        Body::VenueDelivered(_) => true,
        _ => false,
    }
}

/// 引用能指到这一条的平台编号（施工 V-39 上）：场所的 `message.user`（睡着时收到的、补拉回来的也算）和 `venue.delivered`
/// 的编号；别的没有。
pub fn quoted_as(event: &Event) -> Option<&str> {
    match &event.body {
        Body::MessageUser(said) => said.venue.as_ref().map(|venue| venue.msg.as_str()),
        Body::VenueDelivered(delivered) => Some(delivered.msg.as_str()),
        _ => None,
    }
}

/// 这一条引用的平台编号；不是场所的 `message.user`、不引用的没有。
fn reply_to(event: &Event) -> Option<&str> {
    match &event.body {
        Body::MessageUser(said) => said.venue.as_ref()?.reply_to.as_deref(),
        _ => None,
    }
}

/// 撤回的那一条：平台编号、谁撤的。
fn recall(event: &Event) -> Option<(&str, &ExternalId)> {
    match &event.body {
        Body::VenueRecalled(recalled) => Some((recalled.msg.as_str(), &recalled.by)),
        _ => None,
    }
}

/// 之前的记录里的一行，不带换行：撤回了的照 `recalled` 打记号。不是记录的（[`on_record`]）没有。
fn line(
    event: &Event,
    recalled: &BTreeMap<&str, &ExternalId>,
    texts: &GroupChat,
    forward: Option<&ForwardTexts>,
) -> Option<String> {
    match &event.body {
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
    }
}

/// 引用的那一条接在编号后面的：` 发的人: 内容`，她发的 ` [you]: 内容`；引用指不到的（[`quoted_as`]）没有。
fn quote_of(event: &Event, texts: &GroupChat) -> Option<String> {
    match &event.body {
        Body::MessageUser(said) => said.venue.as_ref().map(|venue| {
            format!(
                " {}: {}",
                sender(&event.by, venue),
                content(&said.blocks, &venue.media, texts)
            )
        }),
        Body::VenueDelivered(delivered) => Some(format!(" [you]: {}", you_said(delivered, texts))),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
