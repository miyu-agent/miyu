//! 「哪个账号的哪个任务变了」的广播（施工 J-1 下）：建、停、续、删、改下一次、到点交过都报一声，订阅着 `schedules` 的连接
//! 只推自己账号的（`subscriptions/listed.rs`）。推的是这一刻的整项，晚到的、重复的都对。一条是「账号、换行、编号」，同项目的。

use tokio::sync::broadcast;

use super::Schedules;

/// 广播里最多攒几条没收的：到点的一次交几个，人一次改一个。
pub(super) const CHANGES: usize = 64;

impl Schedules {
    /// 收「哪个任务变了」的一头。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<String> {
        self.changes.subscribe()
    }

    /// 广播任务 `id` 变了。
    pub(super) fn notify(&self, id: &str) {
        if self.changes.send(id.to_string()).is_err() {
            // 没有订阅着的连接：不用说。
        }
    }
}
