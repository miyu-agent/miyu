//! 照记忆日志算出的底账（施工 R-3 上；施工 R-6 修从 `memory.rs` 挪出来，那边只剩事件怎么写、怎么读）：一条现在的样子
//! [`Entry`]，整间的 [`MemoryBook`]。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::event::Event;
use miyu_kernel::id::{CommandId, Seq, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use super::{Cleared, MemoryEvent, MemoryId, Source, from_event};

/// 底账里的一条：现在的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 编号。
    pub id: MemoryId,
    /// 类。
    pub class: String,
    /// 正文。
    pub text: String,
    /// 出处。
    pub sources: Vec<Source>,
    /// 听众。
    pub audience: Vec<By>,
    /// 说的是哪天的事。
    pub about: Option<String>,
    /// 它改的是哪一条（施工 R-7 上：合并照它认出自己改出来的那几条）；新记的没有。
    pub replaces: Option<MemoryId>,
    /// 在哪个场所说的（施工 R-13 上）；主人那边记的、以前记的没有。
    pub venue: Option<VenueId>,
    /// 只用来检索的关键词（施工 R-15 上）；没有的没有。
    pub keywords: Option<String>,
    /// 记下的时刻。
    pub at: Timestamp,
    /// 谁记的。
    pub by: By,
    /// 被哪一条改掉了。
    pub replaced_by: Option<MemoryId>,
    /// 作废了的为什么。
    pub retired: Option<String>,
    /// 作废的时刻（施工 R-6 修：抽取照它挑这一段以后忘掉的）；没作废的、恢复了的没有。
    pub retired_at: Option<Timestamp>,
    /// 清掉了（施工 R-3 补）：哪里都不出来，作废的一起也不出来。
    pub cleared: bool,
}

impl Entry {
    /// 现在还算数：没被改掉、没作废、没清掉。出处活不活另看（回合库的墓碑，`memory.md` 第二条第 4 款）。
    pub fn current(&self) -> bool {
        self.replaced_by.is_none() && self.retired.is_none() && !self.cleared
    }
}

/// 照记忆日志算出的底账。
#[derive(Debug, Default)]
pub struct MemoryBook {
    entries: BTreeMap<MemoryId, Entry>,
    /// 带命令编号（`cause`）的每一条做成了什么：那一条的编号、清掉几条（施工 R-3 补，04 第六节第 1 条：同一个编号只生效一次）。
    done: BTreeMap<CommandId, (MemoryId, usize)>,
    /// 每个会话抽到了哪（施工 R-6 上）：最大的那个 `upto`。
    extracted: BTreeMap<SessionId, Seq>,
    /// 算数的那一份摘要和它合到第几条（施工 R-7 上、下）：不算了的没有。
    summary: Option<(String, Seq)>,
    /// 摘要不算了、还没写新的一份（施工 R-7 下）：下一次合并从头来。
    stale: bool,
    /// 最后一次合并：那一条的时刻、合到第几条（施工 R-7 上）。
    merged: Option<(Timestamp, Seq)>,
    /// 最后一次合并以后抽过的会话（施工 R-7 上）：够数了才再合。
    since_merge: BTreeSet<SessionId>,
    /// 开始合了、还没收尾的那一批合到第几条（施工 R-17）。
    merging: Option<Seq>,
}

impl MemoryBook {
    /// 看一条事件，交回清掉了几条（清空的；不算改掉的旧版本，别的是 0）。不是记忆的跳过。
    ///
    /// # Errors
    ///
    /// 是记忆的、`body` 读不懂：交回为什么，调的一方记日志，底账照旧。
    pub fn see(&mut self, event: &Event) -> Result<usize, String> {
        let Some(cleared) = self.apply(event)? else {
            return Ok(0);
        };
        if let Some(cause) = &event.cause {
            self.done
                .insert(cause.clone(), (MemoryId::new(event.seq), cleared));
        }
        Ok(cleared)
    }

    /// 会话 `session` 抽到了会话日志的第几条（施工 R-6 上）；还没抽过的是 `None`。
    pub fn extracted(&self, session: &SessionId) -> Option<Seq> {
        self.extracted.get(session).copied()
    }

    /// 算数的那一份摘要和它合到记忆日志的第几条（施工 R-7 上、下，`memory.md` 第七条第 8 款）：还没合并过的、不算了的没有。
    pub fn summary(&self) -> Option<(&str, Seq)> {
        self.summary
            .as_ref()
            .map(|(text, upto)| (text.as_str(), *upto))
    }

    /// 摘要写下以后改了、作废了它合进去的一条，或者清空了，还没写新的一份（施工 R-7 下）：下一次合并照没合并过的从头来。
    pub fn summary_stale(&self) -> bool {
        self.stale
    }

    /// 编号 `id` 的那一条合进了现在的摘要：它变了，摘要就不算了。
    fn touch(&mut self, id: MemoryId) {
        if self
            .summary
            .as_ref()
            .is_some_and(|(_, upto)| id.seq() <= *upto)
        {
            self.summary = None;
            self.stale = true;
        }
    }

    /// 最后一次合并：那一条的时刻、合到记忆日志的第几条（施工 R-7 上）；还没合并过的没有。
    pub fn merged(&self) -> Option<(Timestamp, Seq)> {
        self.merged
    }

    /// 开始合了（`ext.memory.merging`）、还没收尾（同一个 `upto` 的 `merged`、`merge_failed`）的那一批合到第几条（施工 R-17）。
    pub fn merging(&self) -> Option<Seq> {
        self.merging
    }

    /// 最后一次合并以后抽过几个会话（施工 R-7 上，`memory.md` 第七条第 1 款）；没合并过的照全部，同一个会话算一个。
    pub fn sessions_since_merge(&self) -> usize {
        self.since_merge.len()
    }

    /// 编号 `id` 的那一条什么时候忘掉的（施工 R-6 修，`memory.md` 第六条第 4 款）：作废的照作废的时刻，改掉的照新版本记下的
    /// 时刻；还算数的、恢复了的、没有的没有。清掉的照它清掉以前的样子交回，算不算由调的一方看。
    pub fn dropped_at(&self, id: MemoryId) -> Option<Timestamp> {
        let entry = self.entries.get(&id)?;
        entry.retired_at.or_else(|| {
            entry
                .replaced_by
                .and_then(|newer| self.entries.get(&newer))
                .map(|newer| newer.at)
        })
    }

    /// 命令编号 `cause` 做过的：那一条的编号、清掉几条；没做过的是 `None`。
    pub fn done(&self, cause: &CommandId) -> Option<(MemoryId, usize)> {
        self.done.get(cause).copied()
    }

    /// 算一条，交回清掉了几条（不是清空的是 0）；不是记忆的交回 `None`。
    fn apply(&mut self, event: &Event) -> Result<Option<usize>, String> {
        match from_event(event) {
            None => Ok(None),
            Some(Err(why)) => Err(why),
            Some(Ok(MemoryEvent::Saved(saved))) => {
                let id = MemoryId::new(event.seq);
                if let Some(old) = saved.replaces {
                    self.touch(old);
                }
                if let Some(old) = saved.replaces.and_then(|old| self.entries.get_mut(&old)) {
                    old.replaced_by = Some(id);
                }
                self.entries.insert(
                    id,
                    Entry {
                        id,
                        class: saved.class,
                        text: saved.text,
                        sources: saved.sources,
                        audience: saved.audience,
                        about: saved.about,
                        venue: saved.venue,
                        keywords: saved.keywords,
                        replaces: saved.replaces,
                        at: event.at,
                        by: event.by.clone(),
                        replaced_by: None,
                        retired: None,
                        retired_at: None,
                        cleared: false,
                    },
                );
                Ok(Some(0))
            }
            Some(Ok(MemoryEvent::Retired(retired))) => {
                self.touch(retired.id);
                if let Some(entry) = self.entries.get_mut(&retired.id) {
                    entry.retired = Some(retired.why);
                    entry.retired_at = Some(event.at);
                }
                Ok(Some(0))
            }
            // 关键词不进摘要（只用来检索）：摘要照旧算数。
            Some(Ok(MemoryEvent::Tagged(tagged))) => {
                if let Some(entry) = self.entries.get_mut(&tagged.id) {
                    entry.keywords = (!tagged.keywords.is_empty()).then_some(tagged.keywords);
                }
                Ok(Some(0))
            }
            // 同作废：摘要合进去的那一条变了，摘要就不算了（第七条第 8 款）。
            Some(Ok(MemoryEvent::Restored(restored))) => {
                self.touch(restored.id);
                if let Some(entry) = self.entries.get_mut(&restored.id) {
                    entry.retired = None;
                    entry.retired_at = None;
                }
                Ok(Some(0))
            }
            Some(Ok(MemoryEvent::Cleared(cleared))) => {
                if self.summary.take().is_some() {
                    self.stale = true;
                }
                // 交回的条数不算改掉的旧版本：它们本来就看不见，人数的是看得见的那些（作废的带 `forgotten` 看得见）。
                let mut shown = 0;
                for id in self.clears(&cleared) {
                    if let Some(entry) = self.entries.get_mut(&id) {
                        entry.cleared = true;
                        shown += usize::from(entry.replaced_by.is_none());
                    }
                }
                Ok(Some(shown))
            }
            // 照最大的记：日志是先后写的，取最大的也不怕哪一条重了。
            Some(Ok(MemoryEvent::Extracted(extracted))) => {
                let upto = self
                    .extracted
                    .entry(extracted.session.clone())
                    .or_insert(extracted.upto);
                *upto = (*upto).max(extracted.upto);
                self.since_merge.insert(extracted.session);
                Ok(Some(0))
            }
            Some(Ok(MemoryEvent::Summary(summary))) => {
                self.summary = Some((summary.text, summary.upto));
                self.stale = false;
                Ok(Some(0))
            }
            Some(Ok(MemoryEvent::Merged(merged))) => {
                self.merged = Some((event.at, merged.upto));
                self.since_merge.clear();
                self.close(merged.upto);
                Ok(Some(0))
            }
            Some(Ok(MemoryEvent::Merging(merging))) => {
                self.merging = Some(merging.upto);
                Ok(Some(0))
            }
            Some(Ok(MemoryEvent::MergeFailed(failed))) => {
                self.close(failed.upto);
                Ok(Some(0))
            }
        }
    }

    /// `upto` 那一批收尾了。
    fn close(&mut self, upto: Seq) {
        if self.merging == Some(upto) {
            self.merging = None;
        }
    }

    /// 现在追加 `cleared` 会清掉哪几条，照编号：还没清掉的里，整间的是全部；带会话的是出处全在这个会话里的（照 17 第六节
    /// 撤回的规矩：还有别的出处的不动），人记的（没有出处）不动。记忆库照它删字。
    pub fn clears(&self, cleared: &Cleared) -> Vec<MemoryId> {
        self.entries
            .values()
            .filter(|entry| !entry.cleared)
            .filter(|entry| match &cleared.session {
                None => true,
                Some(session) => {
                    !entry.sources.is_empty()
                        && entry
                            .sources
                            .iter()
                            .all(|source| &source.session == session)
                }
            })
            .map(|entry| entry.id)
            .collect()
    }

    /// 编号 `id` 的那一条；没有的是 `None`。
    pub fn get(&self, id: MemoryId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    /// 全部，照编号：改掉的、作废的、清掉的也在。
    pub fn all(&self) -> impl DoubleEndedIterator<Item = &Entry> {
        self.entries.values()
    }
}
