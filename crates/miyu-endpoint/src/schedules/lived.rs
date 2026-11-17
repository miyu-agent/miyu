//! 人生的两样小账在端点这一层的口子（施工 LF-14，`docs/blueprint/life.md`「推演轮数」「结束了、还没看过」）：存在定时库的
//! 两张小表里（`miyu_store::schedules` 的 `lived.rs`），这里只是照 [`Schedules`] 的规矩开表、转错。表用不了的照错报，调的
//! 一方自己定要不要紧。

use miyu_kernel::id::SessionId;
use miyu_store::schedules::Unseen;

use super::{ScheduleError, Schedules};

impl Schedules {
    /// 人生那一行 `id` 推演到第几轮：每次到点交进会话算一轮（`run.rs`）。一轮都没有的是 0。
    ///
    /// # Errors
    ///
    /// 表用不了、读不了。
    pub fn rounds(&self, id: &str) -> Result<u64, ScheduleError> {
        Ok(self.table(false)?.rounds(id)?)
    }

    /// 她自己结束了、还没人看过的那几段人生，结束得早的在前。
    ///
    /// # Errors
    ///
    /// 表用不了、读不了。
    pub fn unseen(&self) -> Result<Vec<Unseen>, ScheduleError> {
        Ok(self.table(false)?.unseen()?)
    }

    /// 记下一段还没人看过的人生。
    ///
    /// # Errors
    ///
    /// 表用不了、写不了。
    pub(crate) fn mark_unseen(&self, one: &Unseen) -> Result<(), ScheduleError> {
        Ok(self.table(false)?.mark_unseen(one)?)
    }

    /// 会话 `session` 的那一段看过了：清掉，原来记着的交回真。
    ///
    /// # Errors
    ///
    /// 表用不了、写不了。
    pub(crate) fn seen(&self, session: &SessionId) -> Result<bool, ScheduleError> {
        Ok(self.table(false)?.seen(session)?)
    }
}
