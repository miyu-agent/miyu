//! 写关键词要的两件（施工 R-15 下，`memory/tag.rs`）：挑还没有关键词的，落下写好的。

use miyu_kernel::id::ModuleId;
use miyu_kernel::origin::{By, Module};
use miyu_kernel::time::Timestamp;
use miyu_recall::{Entry, MemoryEvent, MemoryId, Tagged};
use miyu_tool::Refused;

use super::{Keeper, Stamp};

impl Keeper {
    /// 这一间现在算数、听众合、还没有关键词的，最多 `limit` 条：`first` 里的照先后在前，再照新的在前补满。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了。
    pub(crate) fn untagged(&self, first: &[MemoryId], limit: usize) -> Result<Vec<Entry>, Refused> {
        let log = self.log()?;
        // 算不算数、听众、出处另查（`shown`：改掉的、作废的、清掉的都不算）。
        let bare = |entry: &Entry| entry.keywords.is_none();
        let (mut picked, mut rest): (Vec<Entry>, Vec<Entry>) = log.book(|book| {
            let picked = first
                .iter()
                .filter_map(|id| book.get(*id))
                .filter(|entry| bare(entry))
                .cloned()
                .collect();
            let rest = book.all().filter(|entry| bare(entry)).cloned().collect();
            (picked, rest)
        });
        // 听众、出处活不活另查（会看回合库），不在拿着底账的时候查。
        picked.retain(|entry| self.shown(entry, false));
        let mut seen = std::collections::BTreeSet::new();
        picked.retain(|entry| seen.insert(entry.id));
        rest.retain(|entry| !seen.contains(&entry.id));
        rest.sort_by_key(|entry| std::cmp::Reverse(entry.id));
        for entry in rest {
            if picked.len() >= limit {
                break;
            }
            if self.shown(&entry, false) {
                picked.push(entry);
            }
        }
        picked.truncate(limit);
        Ok(picked)
    }

    /// 落下写好的关键词，交回写上了几条：那一条还算数、听众合、还没有关键词的才写（人改过的、她当场给了的不盖），`by` 是
    /// 记忆模块。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进。
    pub(crate) fn record_tags(
        &self,
        at: Timestamp,
        tags: Vec<(MemoryId, String)>,
    ) -> Result<u32, Refused> {
        let log = self.log()?;
        let by = By::Module(Module {
            id: ModuleId::parse(crate::memory::MODULE).expect("合模块编号的写法"),
        });
        let mut count = 0;
        for (id, keywords) in tags {
            let entry = log.book(|book| book.get(id).cloned());
            let Some(entry) = entry.filter(|entry| entry.keywords.is_none()) else {
                continue;
            };
            if !self.shown(&entry, false) {
                continue;
            }
            let stamp = Stamp {
                at,
                by: by.clone(),
                cause: None,
            };
            self.append(&log, stamp, &MemoryEvent::Tagged(Tagged { id, keywords }))?;
            count += 1;
        }
        Ok(count)
    }
}
