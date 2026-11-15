//! 她记下的自己的变化（施工 LF-11，`docs/blueprint/memory.md`「事件」的类、「工具」，设计 `70-推演人生.md` 第六节）：一间记忆里
//! 三条路分开在这一层，工具不判自己在哪种会话里（只在人生会话里有 `note_change`，那是工具面的事）。
//!
//! - 她在人生会话里的 `note_change`：[`Keeper::note`] 记一条、改一条，[`Keeper::drop_note`] 去掉一条；只碰类是
//!   [`CHANGE`] 的，现在算数的最多 [`CHANGES`] 条。
//! - 她的 `remember` 的 `replaces`、`forget`（[`Keeper::forget`]），人经协议改一条（`memory.update`）：指着一条 [`CHANGE`]
//!   的拒成 [`Refused::Change`]（[`Keeper::save`]、[`Keeper::update`] 先问 `not_a_change`）。
//! - 人经协议作废（`memory.forget` 走 [`Keeper::retire`]）、恢复、清空照常，不拦：主人只能看、作废、恢复，不能替她写。

use miyu_recall::{CHANGE, CHANGES, MemoryEvent, MemoryId, Retired, Saved, Source, Why};
use miyu_store::memory::MemoryLog;
use miyu_tool::{NoteRefused, Refused};

use super::{Keeper, Stamp};

impl Keeper {
    /// 记一条她自己的变化（`note_change` 给了 `text`）：出处 `sources`，听众是这次的听众；写了 `replaces` 的是改那一条，它得是
    /// 现在算数的一条 [`CHANGE`]；`why` 是她写的原因，记在这一条上。新记的在现在算数的（没改掉、没作废、没清掉、出处活着）已经有 [`CHANGES`] 条时拒。没有是谁、
    /// 涉及谁、关键词、群名。交回新的编号。
    ///
    /// # Errors
    ///
    /// `replaces` 没有这一条、不是变化、已经不算了；已经满了；记忆日志开不了、写不进。
    pub fn note(
        &self,
        stamp: Stamp,
        (text, why): (String, Option<String>),
        replaces: Option<MemoryId>,
        sources: Vec<Source>,
    ) -> Result<MemoryId, NoteRefused> {
        let log = self.log()?;
        match replaces {
            Some(old) => self.current_note(&log, old)?,
            None => {
                let kept = log.book(|book| {
                    book.all()
                        .filter(|entry| entry.class == CHANGE && self.shown(entry, false))
                        .count()
                });
                if kept >= CHANGES {
                    return Err(NoteRefused::Full(CHANGES));
                }
            }
        }
        let (audience, venue) = self.listener.heard();
        let saved = Saved {
            class: CHANGE.to_string(),
            text,
            sources,
            audience,
            replaces,
            about: None,
            venue,
            keywords: None,
            who: None,
            people: Vec::new(),
            place: None,
            why: why.map(|why| Box::new(Why(why))),
            imported: None,
        };
        let (id, _) = self.append(&log, stamp, &MemoryEvent::Saved(saved))?;
        Ok(id)
    }

    /// 去掉一条她自己的变化（`note_change` 给了 `drop`）：它得是现在算数的一条 [`CHANGE`]；`why` 记进作废的原因。
    ///
    /// # Errors
    ///
    /// 没有这一条、不是变化、已经不算了；记忆日志开不了、写不进。
    pub fn drop_note(&self, stamp: Stamp, id: MemoryId, why: String) -> Result<(), NoteRefused> {
        let log = self.log()?;
        self.current_note(&log, id)?;
        let retired = Retired {
            id,
            why,
            kind: None,
            kept: None,
        };
        self.append(&log, stamp, &MemoryEvent::Retired(retired))?;
        Ok(())
    }

    /// 她的 `forget`：同 [`Keeper::retire`]，指着一条 [`CHANGE`] 的拒（那一条照旧算数）。
    ///
    /// # Errors
    ///
    /// 是她记下的变化；别的同 [`Keeper::retire`]。
    pub fn forget(&self, stamp: Stamp, id: MemoryId, why: String) -> Result<(), Refused> {
        let log = self.log()?;
        self.not_a_change(&log, id)?;
        self.retire(stamp, id, why)
    }

    /// 编号 `id` 的那一条不是她记下的变化：是的拒成 [`Refused::Change`]；没有的不管（由调的一方照没有说）。
    pub(super) fn not_a_change(&self, log: &MemoryLog, id: MemoryId) -> Result<(), Refused> {
        match log.book(|book| book.get(id).is_some_and(|entry| entry.class == CHANGE)) {
            true => Err(Refused::Change(id)),
            false => Ok(()),
        }
    }

    /// 编号 `id` 的那一条是现在算数的一条变化：没有的当没有，别的类说不是，改掉的、作废的、清掉的说已经不算了。
    fn current_note(&self, log: &MemoryLog, id: MemoryId) -> Result<(), NoteRefused> {
        let class = log.book(|book| book.get(id).map(|entry| entry.class.clone()));
        match class {
            None => Err(Refused::NoSuch(id).into()),
            Some(class) if class != CHANGE => Err(NoteRefused::NotANote(id)),
            Some(_) => self.current(log, id).map(|_| ()).map_err(Into::into),
        }
    }
}
