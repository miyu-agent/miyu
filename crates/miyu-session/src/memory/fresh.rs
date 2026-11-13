//! 醒来时交新记的（施工 LF-6，`docs/blueprint/memory.md` 第三条第 9 款，设计 `70-推演人生.md` 第三节第 8 条）：人生会话一直开着，
//! 常驻那一块只在第一轮、压缩以后交一次，人生循环叫醒的一轮又不联想，中途别处记下的事她一直不知道。人生循环叫醒的那一轮开头，
//! 挂接点多交一块「这一段上下文里交过以后新记的」。
//!
//! - 哪些回合：开这一轮的那条消息是人生循环发的（内核交来的 `opener`）。别的都没有。
//! - 从哪条以后：这一段上下文里常驻那一块、以前这种块列过的最新那一条（照 `present` 的 `refs`，连同这一轮常驻那一块列的）；
//!   只有摘要一行、一条都没列的，照底账现在的摘要合到哪。联想带过的只用来去重、不抬下限：它会带回很新的一条，照它算的话比它
//!   旧、她没见过的那几条就永远不交了。
//! - 去掉：作废的、改掉的、清掉的、出处死了的（同常驻那一块，[`Keeper::list`]）；合并自己改出来的；出处全在这个会话自己的（她
//!   在这里过的，上下文里本来就有）。
//! - 写法同常驻那一块（外壳、一行、上限、截了的那一句，`summary.rs` 的 `listed`），类别是 `fresh`。一条都没有的不交。

use std::collections::BTreeSet;

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId, SessionId};
use miyu_kernel::origin::{By, ScheduleLoop};
use miyu_kernel::session::Injection;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{Entry, MemoryId};

use super::merge::made_by_merge;
use super::summary::listed;
use super::{Filter, Keeper};

/// 注入那一块的模块：和常驻那一块同一个 `memory`。
const MODULE: &str = "memory";

/// 常驻那一块的类别。
const SUMMARY: &str = "memory";

/// 这一块的类别：`fresh`。`present` 照类别和常驻那一块、联想分得开。
const KIND: &str = "fresh";

/// 这一轮是人生循环叫醒的：开这一轮的那条消息（`opener`）是定时表里 `loop` 是 `life` 的那一种发的。
pub(crate) fn woken_by_life(opener: Option<&By>) -> bool {
    matches!(opener, Some(By::Schedule(schedule)) if schedule.looping == Some(ScheduleLoop::Life))
}

impl Keeper {
    /// 人生循环叫醒的这一轮多交的那一块（`present` 是这一段上下文里模块注入过的，连同这一轮常驻那一块；`session` 是这个会话，
    /// `offset` 是它的时区）。人格记忆没装的、核心没读到外壳的字的、这一段没有常驻那一块的（这一轮该交却没交成：超时、读错，
    /// 下次醒来再说）、一条新的都没有的、读不了的不交。碰磁盘，调的一方放在阻塞线程里。
    pub(crate) fn fresh(
        &self,
        offset: UtcOffset,
        present: &[Present],
        session: &SessionId,
    ) -> Option<Injection> {
        if !self.installed() {
            return None;
        }
        let texts = self.memory.summary.as_ref()?;
        let summarized = present
            .iter()
            .any(|present| present.module.as_str() == MODULE && present.kind.as_str() == SUMMARY);
        if !summarized {
            return None;
        }
        let filter = Filter {
            class: None,
            from: None,
            forgotten: false,
            limit: usize::MAX,
        };
        let read = self.list(&filter).and_then(|entries| {
            let digest = self
                .log()
                .map_err(|refused| format!("{refused:?}"))?
                .book(|book| book.summary().map(|(_, upto)| upto));
            Ok((entries, digest))
        });
        let (mut entries, digest) = match read {
            Ok(read) => read,
            Err(error) => {
                tracing::warn!(target: crate::TARGET, error = error.as_str(), "memory fresh not read");
                return None;
            }
        };
        let since = since(present, digest.map(MemoryId::new));
        entries.retain(|entry| wanted(entry, &since, session));
        if entries.is_empty() {
            return None;
        }
        let (text, refs) = listed(texts, None, &entries, offset);
        Some(Injection {
            module: ModuleId::parse(MODULE).expect("合写法"),
            fact: ContextInjected {
                kind: FactKind::parse(KIND).expect("合写法"),
                text,
                refs,
                after: false,
            },
        })
    }
}

/// 从哪条以后交、哪几条交过：下限是常驻那一块、以前这种块列过的最新那一条，一条都没列过的照摘要合到的那一条 `digest`（都没有
/// 的没有下限）；交过的是模块 `memory` 所有块的 `refs`（联想带过的在这里，只去重）。人卡那一块的 `refs` 是平台编号，认不成
/// 记忆的编号，自然不算。
fn since(present: &[Present], digest: Option<MemoryId>) -> (Option<MemoryId>, BTreeSet<MemoryId>) {
    let ours = || {
        present
            .iter()
            .filter(|present| present.module.as_str() == MODULE)
    };
    let ids = |present: &Present| -> Vec<MemoryId> {
        present
            .refs
            .iter()
            .filter_map(|id| MemoryId::parse(id))
            .collect()
    };
    let floor = ours()
        .filter(|present| matches!(present.kind.as_str(), SUMMARY | KIND))
        .flat_map(ids)
        .max()
        .or(digest);
    let seen = ours().flat_map(ids).collect();
    (floor, seen)
}

/// 这一条要交：比下限新、这一段没交过（`since` 是 [`since`] 交回的）、不是合并自己改出来的、出处不全在这个会话 `session` 里。
/// 算不算数、出处活不活由 [`Keeper::list`] 挑过。
fn wanted(
    entry: &Entry,
    (floor, seen): &(Option<MemoryId>, BTreeSet<MemoryId>),
    session: &SessionId,
) -> bool {
    floor.is_none_or(|floor| entry.id > floor)
        && !seen.contains(&entry.id)
        && !made_by_merge(entry)
        && !lived_here(entry, session)
}

/// 出处全在这个会话 `session` 里（她在这里过的，上下文里本来就有）。人记的没有出处，不算。
fn lived_here(entry: &Entry, session: &SessionId) -> bool {
    !entry.sources.is_empty()
        && entry
            .sources
            .iter()
            .all(|source| &source.session == session)
}

#[cfg(test)]
mod tests;
