//! 造会话、载入时定下的几样：会话认不认几条跟着包走的命令（施工 X-8 下的 `/goal`、施工 J-2 的 `/loop`，照工具面定下，中途换了
//! 预设下次载入照新的），关没关起来，记忆的那一间。

use super::Handle;

impl Handle {
    /// 带上这个会话的记忆的那一间（施工 R-3 补）。
    #[must_use]
    pub(crate) fn with_memory(mut self, memory: crate::memory::RoomCell) -> Handle {
        self.memory = memory;
        self
    }

    /// 带上这个会话关没关起来（施工 SU-4 中）。
    #[must_use]
    pub(crate) fn with_confined(mut self, confined: bool) -> Handle {
        self.confined = confined;
        self
    }

    /// 这个会话关没关起来（施工 SU-4 中）：群、陌生人的私聊是；本机的、主人本人的私聊不是。
    pub fn confined(&self) -> bool {
        self.confined
    }

    /// 带上这个会话有没有长期目标（施工 X-8 下）。
    #[must_use]
    pub(crate) fn with_goals(mut self, goals: bool) -> Handle {
        self.goals = goals;
        self
    }

    /// 这个会话有没有长期目标（施工 X-8 下）：工具面上有 `goal`（本机的主会话、长期目标装了、预设开着）。
    pub fn goals(&self) -> bool {
        self.goals
    }

    /// 带上认不认 `/loop`（施工 J-2）。
    #[must_use]
    pub(crate) fn with_loops(mut self, loops: bool) -> Handle {
        self.loops = loops;
        self
    }

    /// 认不认 `/loop`（施工 J-2）：工具面上有 `schedule_wakeup`。
    pub fn loops(&self) -> bool {
        self.loops
    }

    /// 带上和 actor 共用的文件改动的版本（施工 OV-1 下）。
    #[must_use]
    pub(crate) fn with_prefix(mut self, prefix: std::sync::Arc<crate::prefix::Prefix>) -> Handle {
        self.prefix = prefix;
        self
    }

    /// 文件改动的版本：会话看到的、人放行过的（施工 OV-1 下）。
    pub fn prefix(&self) -> &crate::prefix::Prefix {
        &self.prefix
    }

    /// 带上和 actor 共用的「在不在等人」（施工 SL-1）。
    #[must_use]
    pub(crate) fn with_waiting(
        mut self,
        waiting: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Handle {
        self.waiting = waiting;
        self
    }

    /// 在不在等人确认、回答（施工 SL-1）：会话列表照它写 `waiting`。会话停了的照最后一次写的。
    pub fn waiting(&self) -> bool {
        self.waiting.load(std::sync::atomic::Ordering::Acquire)
    }

    /// 包 `package` 当前的状态（施工 TW-1）：没写过、清掉了的没有。落了盘推出去以后的。
    pub fn package_state(&self, package: &str) -> Option<miyu_kernel::event::State> {
        self.shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .states
            .get(package)
            .cloned()
    }
}
