//! 会话认不认几条跟着包走的命令（施工 X-8 下的 `/goal`、施工 J-2 的 `/loop`）：造会话、载入时照工具面定下，中途换了预设下次
//! 载入照新的。

use super::Handle;

impl Handle {
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
}
