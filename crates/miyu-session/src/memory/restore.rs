//! 恢复作废的（施工 R-12 下，`docs/blueprint/memory.md` 第二条第 6 款）：人恢复误删的，追加 `ext.memory.restored`，那一条
//! 不算作废了。只恢复作废的：改掉的旧版本、清掉的不恢复（清掉的哪里都不出来，第二条第 5 款）；本来就算数的什么都不写。

use miyu_recall::{MemoryEvent, MemoryId, Restored};
use miyu_tool::Refused;

use super::keeper::done;
use super::{Keeper, Stamp};

impl Keeper {
    /// 恢复作废了的一条：本来就算数的什么都不写；这条命令做过的交回那一次的（记忆日志认编号）。
    ///
    /// # Errors
    ///
    /// 没有这一条（听众不合的也当没有）；改掉了、清掉了；记忆日志开不了、写不进。
    pub fn restore(&self, stamp: Stamp, id: MemoryId) -> Result<(), Refused> {
        let log = self.log()?;
        if done(&log, &stamp).is_some() {
            return Ok(());
        }
        let entry = log
            .book(|book| book.get(id).cloned())
            .filter(|entry| self.hears(entry))
            .ok_or(Refused::NoSuch(id))?;
        if entry.replaced_by.is_some() || entry.cleared {
            return Err(Refused::NotCurrent(id));
        }
        if entry.retired.is_none() {
            return Ok(());
        }
        self.append(&log, stamp, &MemoryEvent::Restored(Restored { id }))
            .map(|_| ())
    }
}
