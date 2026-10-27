//! 合并的那一间（施工 R-7 上，`docs/blueprint/memory.md` 第七条第 1、4、6 款）：够不够合、合什么，记下一次合并、记一条失败的
//! 记号。碰记忆日志，调的一方放在阻塞线程里。

use std::time::Duration;

use miyu_kernel::id::{ModuleId, Seq};
use miyu_kernel::origin::{By, Module};
use miyu_kernel::time::Timestamp;
use miyu_recall::merge::{Decisions, Drafted, names};
use miyu_recall::{
    Card, Entry, INTERRUPTED, MemoryEvent, MemoryId, MergeFailed, Merged, Merging, Retired, Saved,
    Summary, Swept,
};
use miyu_store::memory::MemoryLog;
use miyu_tool::Refused;

use super::{Batch, Sweep};

/// 一趟整理顺带重看几条旧记忆（施工 R-19，2026-10-19 项目主人照推荐定）：照编号轮，轮完一圈从头来。
pub(crate) const SWEEP: usize = 20;
use crate::memory::{Keeper, Stamp};

/// 记忆模块：合并记下的 `by`。
fn by() -> By {
    By::Module(Module {
        id: ModuleId::parse(crate::memory::MODULE).expect("合模块编号的写法"),
    })
}

impl Keeper {
    /// 这一间够不够合、合什么（`memory.md` 第七条第 1 款）：`check` 是间隔和会话数，没有的不看（接着合剩下的那几次）。
    /// `sweep` 是挑（一趟的头一批，施工 R-19，第 11 款）的再带上轮到的旧记忆：上次重看到的那一条后面、现在算数、听众合的，
    /// 照编号最多 [`SWEEP`] 条，不够的从头接；人叫的整理没有新记的也照样重看这几条，后台的没有新记的不合。
    /// 没有新记的、不够的交回没有。新记的是上次合到的那一条以后、现在算数、听众合、出处活着的，不算合并自己改出来的；摘要
    /// 不算了的（第 8 款，施工 R-7 下）照没合并过的从头来：现在算数的都是新记的，摘要照还没有。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了。
    pub(super) fn merge_batch(
        &self,
        now: Timestamp,
        check: Option<(Duration, usize)>,
        sweep: Sweep,
    ) -> Result<Option<Batch>, Refused> {
        let log = self.log()?;
        let (summary, merged, stale, sessions, all, swept) = log.book(|book| {
            (
                book.summary().map(|(text, _)| text.to_string()),
                book.merged(),
                book.summary_stale(),
                book.sessions_since_merge(),
                book.all().cloned().collect::<Vec<Entry>>(),
                book.swept(),
            )
        });
        if let Some((every, enough)) = check {
            let waited = merged.is_none_or(|(at, _)| {
                now.unix_millis().saturating_sub(at.unix_millis())
                    >= i64::try_from(every.as_millis()).unwrap_or(i64::MAX)
            });
            if !waited || sessions < enough {
                return Ok(None);
            }
        }
        let after = merged.filter(|_| !stale).map(|(_, upto)| upto);
        let (fresh, older): (Vec<Entry>, Vec<Entry>) = all
            .into_iter()
            .filter(|entry| self.shown(entry, false))
            .partition(|entry| after.is_none_or(|after| entry.id.seq() > after));
        let fresh: Vec<Entry> = fresh
            .into_iter()
            .filter(|entry| !super::made_by_merge(entry))
            .collect();
        // 从头合的（没合并过、摘要不算了的）全是新记的，不用另外重看。
        let sweep = match (sweep, after) {
            (Sweep::Pick, Some(_)) => turn(older, swept, SWEEP),
            // 挪过来的：落以前照旧核对算不算数（`record_merge`）。
            (Sweep::Carry(carried), _) => carried,
            _ => Vec::new(),
        };
        let worth = !fresh.is_empty() || (check.is_none() && !sweep.is_empty());
        Ok(worth.then_some(Batch {
            summary,
            fresh,
            sweep,
            after,
        }))
    }

    /// 记下一次合并（第七条第 4 款）：落的时候还算数、听众合的才落；改的指着旧的、类出处听众照旧，和原文一样的不记；作废的带
    /// 为什么；有摘要的记摘要；人卡（施工 E-1 下，第 15 款）一人一张，叫法接上旧卡的，和现在那一张一样的不记；最后记合到第
    /// `upto` 条、交了 `given` 条。交回改了几条、作废几条。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进。
    pub(super) fn record_merge(
        &self,
        at: Timestamp,
        decided: &Decisions,
        drafted: &[Drafted],
        (upto, given): (Seq, u32),
        swept: Option<&Swept>,
    ) -> Result<(u32, u32), Refused> {
        let log = self.log()?;
        let stamp = || Stamp {
            at,
            by: by(),
            cause: None,
        };
        let now = |id: MemoryId| {
            log.book(|book| book.get(id).cloned())
                .filter(|entry| entry.current() && self.shown(entry, false))
        };
        let (mut revised, mut retired) = (0, 0);
        for one in &decided.revised {
            let Some(old) = now(one.id) else { continue };
            if old.text == one.text {
                continue;
            }
            let saved = Saved {
                class: old.class,
                text: one.text.clone(),
                sources: old.sources,
                audience: old.audience,
                replaces: Some(one.id),
                about: old.about,
                venue: old.venue,
                keywords: old.keywords,
                who: old.who,
                people: old.people,
                place: old.place,
            };
            self.append(&log, stamp(), &MemoryEvent::Saved(saved))?;
            revised += 1;
        }
        for one in &decided.retired {
            if now(one.id).is_none() {
                continue;
            }
            let gone = Retired {
                id: one.id,
                why: one.why.clone(),
                kind: Some(one.kind),
                kept: one.by,
            };
            self.append(&log, stamp(), &MemoryEvent::Retired(gone))?;
            retired += 1;
        }
        if let Some(text) = &decided.summary {
            let summary = Summary {
                text: text.clone(),
                upto,
            };
            self.append(&log, stamp(), &MemoryEvent::Summary(summary))?;
        }
        for one in drafted {
            let old = log
                .book(|book| book.card(&one.who).cloned())
                .filter(|card| card.retired.is_none());
            let known = old
                .as_ref()
                .map(|card| card.names.clone())
                .unwrap_or_default();
            let card = Card {
                who: one.who.clone(),
                text: one.text.clone(),
                names: names(&one.who, &one.names, &known),
            };
            if old.is_some_and(|old| old.text == card.text && old.names == card.names) {
                continue;
            }
            self.append(&log, stamp(), &MemoryEvent::Card(card))?;
        }
        let mark = Merged {
            upto,
            given,
            revised,
            retired,
            failed: false,
        };
        self.append(&log, stamp(), &MemoryEvent::Merged(mark))?;
        // 顺带重看到哪（施工 R-19）：合成了才往前走。
        if let Some(swept) = swept {
            self.append(&log, stamp(), &MemoryEvent::Swept(swept.clone()))?;
        }
        Ok((revised, retired))
    }

    /// 开始合一批（施工 R-17，第七条第 10 款）：发请求以前记 `merging`（合到第几条、这一批几条新记的、连它在内还有几条、是不是
    /// 人叫的、交的是哪几条（施工 R-21）、跑在哪个整理会话里（施工 D-2 上））。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进。
    pub(super) fn record_merging(&self, at: Timestamp, merging: Merging) -> Result<(), Refused> {
        let log = self.log()?;
        let stamp = Stamp {
            at,
            by: by(),
            cause: None,
        };
        self.append(&log, stamp, &MemoryEvent::Merging(merging))
            .map(|_| ())
    }

    /// 这一批这一次没合成（施工 R-17）：记为什么、连着第几次；`skip`（连着第三次）的另记一条失败的记号，跳过它（第七条第 6 款）。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进。
    pub(super) fn record_merge_failed(
        &self,
        at: Timestamp,
        upto: Seq,
        given: u32,
        why: &str,
        tries: u32,
        skip: bool,
    ) -> Result<(), Refused> {
        let log = self.log()?;
        let stamp = || Stamp {
            at,
            by: by(),
            cause: None,
        };
        let failed = MergeFailed {
            upto,
            why: why.to_string(),
            tries: Some(tries),
        };
        self.append(&log, stamp(), &MemoryEvent::MergeFailed(failed))?;
        if skip {
            let mark = Merged {
                upto,
                given,
                revised: 0,
                retired: 0,
                failed: true,
            };
            self.append(&log, stamp(), &MemoryEvent::Merged(mark))?;
        }
        Ok(())
    }

    /// 人叫停了（施工 R-22）：`upto` 那一批放弃，记一条 `stopped`，不算没合成、不数次数；下次整理照样从这一批起。
    pub(super) fn record_merge_stopped(&self, at: Timestamp, upto: Seq) -> Result<(), Refused> {
        let log = self.log()?;
        let failed = MergeFailed {
            upto,
            why: miyu_recall::STOPPED.to_string(),
            tries: None,
        };
        let stamp = Stamp {
            at,
            by: by(),
            cause: None,
        };
        self.append(&log, stamp, &MemoryEvent::MergeFailed(failed))?;
        Ok(())
    }

    /// 这一回第一次开这一间（施工 R-17）：上一回开始合了、没收尾的那一批（核心半路停了）补一条 `interrupted`，订阅着的头不会
    /// 一直当它在合。这一回还没有在合的：开日志总在合以前。写不进的记一行，不挡。
    pub(in crate::memory) fn close_interrupted(&self, log: &MemoryLog, at: Timestamp) {
        let Some(upto) = log.book(miyu_recall::MemoryBook::merging) else {
            return;
        };
        let failed = MergeFailed {
            upto,
            why: INTERRUPTED.to_string(),
            tries: None,
        };
        let stamp = Stamp {
            at,
            by: by(),
            cause: None,
        };
        if let Err(refused) = self.append(log, stamp, &MemoryEvent::MergeFailed(failed)) {
            tracing::warn!(target: crate::TARGET, room_kind = self.room().kind(), room_id = self.room_id().as_str(), error = ?refused, "memory merge not recorded");
        }
    }
}

/// 轮到的旧记忆（施工 R-19）：`older` 照编号排，从 `swept` 后面取，最多 `most` 条，不够的从头接（不重复）。
fn turn(older: Vec<Entry>, swept: Option<Seq>, most: usize) -> Vec<Entry> {
    let (before, after): (Vec<Entry>, Vec<Entry>) = older
        .into_iter()
        .partition(|entry| swept.is_some_and(|swept| entry.id.seq() <= swept));
    after.into_iter().chain(before).take(most).collect()
}
