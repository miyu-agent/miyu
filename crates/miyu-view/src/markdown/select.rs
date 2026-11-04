//! 挑要写的条目（施工 SE-1 中下中，`range`）：撤掉的（`hidden`）、撤回的排队消息一律不进；再照范围挑。
//!
//! - `all`：全部。
//! - `context`：她现在的上下文。条目照它引出的那条事件对有效历史（[`super::Sources::context`]）：消息 `m<序号>`、旁白和收尾
//!   `e<序号>`、发出去的图 `s<序号>.<n>` 照那一条；回复里的块 `b<seen>.<块号>` 照 `seen` 找回那条回复（`Sources::replies`）。
//!   检查点那一次压缩（`c<upto>`）`read`、`chat` 照时间线留着、摘要折在下面，`agent` 拿掉、摘要放最前；更早的压缩不进，
//!   检查点以后的（失败了的）照留。还没算上下文的（没交 [`super::Sources::keeping_history`]）当全部。
//! - `{"turns": N}`：最近 N 轮：照条目的 `turn`，从第 N 轮倒数的那一轮的头一条起，后面的都要（不属于哪一轮的旁白也要）。
//!
//! 省掉了几轮：前面整轮都没进的（撤掉的不算）。

use std::collections::BTreeSet;

use miyu_kernel::id::{Seq, TurnId};

use super::options::{Purpose, Range};
use super::{Document, Sources};
use crate::entry::{Body, Entry, EntryId};

/// 挑好的。
pub(super) struct Selection<'a> {
    /// 要写的条目，照先后。
    pub(super) entries: Vec<&'a Entry>,
    /// 前面省掉了几轮。
    pub(super) omitted: u64,
    /// 放在最前面的检查点摘要（`agent` 的 `context`）。
    pub(super) summary: Option<&'a str>,
    /// 是从检查点切开的（`context` 且压缩过）。
    pub(super) compacted: bool,
}

/// 照 `doc` 的范围挑。
pub(super) fn select<'a>(doc: &Document<'a>) -> Selection<'a> {
    let shown: Vec<&'a Entry> = doc.entries.iter().filter(|entry| shown(entry)).collect();
    let all = Selection {
        entries: shown.clone(),
        omitted: 0,
        summary: None,
        compacted: false,
    };
    match doc.options.range {
        Range::All => all,
        Range::Turns(n) => last_turns(shown, n.max(1)),
        Range::Context => match &doc.sources.context {
            None => all,
            Some(context) => {
                let checkpoint = context.upto.map(EntryId::compaction);
                let agent = doc.options.purpose == Purpose::Agent;
                let entries: Vec<&'a Entry> = shown
                    .iter()
                    .copied()
                    .filter(|entry| match compaction_of(&entry.id) {
                        Some(upto) => match context.upto {
                            Some(at) if upto == at => !agent,
                            Some(at) => upto > at,
                            None => true,
                        },
                        None => source(&entry.id, doc.sources)
                            .is_some_and(|seq| context.kept.contains(&seq)),
                    })
                    .collect();
                let summary = match agent {
                    true => checkpoint
                        .as_ref()
                        .and_then(|id| doc.sources.summaries.get(id))
                        .map(String::as_str),
                    false => None,
                };
                Selection {
                    omitted: omitted(&shown, &entries),
                    entries,
                    summary,
                    compacted: context.upto.is_some(),
                }
            }
        },
    }
}

/// 写不写：撤掉的、撤回的排队消息不写。
fn shown(entry: &Entry) -> bool {
    !entry.hidden && !matches!(&entry.body, Body::User(user) if user.withdrawn)
}

/// 最近 `n` 轮。
fn last_turns(shown: Vec<&Entry>, n: u32) -> Selection<'_> {
    let turns = turns_of(shown.iter().copied());
    let keep = usize::try_from(n).unwrap_or(usize::MAX).min(turns.len());
    let first = turns.len() - keep;
    let start = turns
        .get(first)
        .and_then(|turn| shown.iter().position(|entry| entry.turn == Some(*turn)))
        .unwrap_or(shown.len());
    let entries = match keep {
        0 => shown,
        _ => shown[start..].to_vec(),
    };
    Selection {
        entries,
        omitted: u64::try_from(first).unwrap_or(u64::MAX),
        summary: None,
        compacted: false,
    }
}

/// 一条也没进的轮有几轮。
fn omitted(shown: &[&Entry], kept: &[&Entry]) -> u64 {
    let kept: BTreeSet<TurnId> = turns_of(kept.iter().copied()).into_iter().collect();
    let gone = turns_of(shown.iter().copied())
        .into_iter()
        .filter(|turn| !kept.contains(turn))
        .count();
    u64::try_from(gone).unwrap_or(u64::MAX)
}

/// 条目里出现的轮，照头一次出现的先后、不重复。
fn turns_of<'a>(entries: impl Iterator<Item = &'a Entry>) -> Vec<TurnId> {
    let mut seen = BTreeSet::new();
    entries
        .filter_map(|entry| entry.turn)
        .filter(|turn| seen.insert(*turn))
        .collect()
}

/// 压缩那一条（`c<upto>`）替代到哪。
fn compaction_of(id: &EntryId) -> Option<Seq> {
    number(id.as_str().strip_prefix('c')?)
}

/// 条目是哪一条事件引出的：回复里的块照 `seen` 找回那条回复；时间线的一段、只在视图流里有的旁白没有。
fn source(id: &EntryId, sources: &Sources) -> Option<Seq> {
    let id = id.as_str();
    let (kind, rest) = id.split_at(id.char_indices().nth(1).map_or(id.len(), |(at, _)| at));
    let head = rest.split('.').next().unwrap_or(rest);
    match kind {
        "m" | "e" | "s" => number(head),
        "b" => sources.replies.get(&number(head)?).copied(),
        _ => None,
    }
}

/// 写成字的序号。
fn number(text: &str) -> Option<Seq> {
    text.parse::<u64>().ok().and_then(Seq::new)
}
