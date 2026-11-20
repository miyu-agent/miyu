//! 循环等这一轮结束（施工 LO-2，`internal/blueprint/schedules.md` 第三节第 4 条）：跑在已有会话上的循环（固定的、她自己定节奏的）
//! 到点时那个会话正在跑一轮，不交进这一轮，记在这里等它空下来；会话空下来时（`listing` 端口报忙不忙变了，`spawn.rs`）放掉、
//! 叫醒睡着的那个，照常交、由它自己开一轮。
//!
//! 为什么不交进这一轮：交进去的话这一轮的下一步就带着它（「定时交进来的话」照回报的规矩到），她顺着手上的活看到一句「继续」，
//! 不定下一次，循环就停了，那一行停着等她定（2026-11-16 项目主人报的：输出中敲的 `/loop` 图标在、循环不在）。人打断的照样只是
//! 打断这一轮：以前交进去的那句被打断吞掉，现在等它结束再交。
//!
//! 只在内存里记，不碰表：表里那一行还是到点的样子（下一次是过去的时刻），核心重启以后照错过的跳过。定时任务不等，照旧交。

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use miyu_kernel::id::SessionId;

/// 等着的几个：任务编号 → 等哪个会话。
#[derive(Default)]
pub(super) struct Waiting(Mutex<BTreeMap<String, SessionId>>);

impl Waiting {
    /// 任务 `id` 等会话 `session` 空下来。
    pub(super) fn hold(&self, id: &str, session: &SessionId) {
        self.lock().insert(id.to_string(), session.clone());
    }

    /// 不等了（会话已经空着）。
    pub(super) fn drop_one(&self, id: &str) {
        self.lock().remove(id);
    }

    /// 任务 `id` 在等。
    pub(super) fn held(&self, id: &str) -> bool {
        self.lock().contains_key(id)
    }

    /// 会话 `session` 的状态变了：等它的都放掉，有放掉的交回真。还忙着的，到点时再挑出来、再记一次。
    pub(super) fn release(&self, session: &SessionId) -> bool {
        let mut waiting = self.lock();
        let before = waiting.len();
        waiting.retain(|_, on| on != session);
        waiting.len() != before
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, SessionId>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
