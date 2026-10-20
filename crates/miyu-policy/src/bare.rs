//! 裸的快照（施工 D-1 上，设计 `41-整理会话.md` 第二节第 2 条）：整理会话用。没有人格、没有 system、没有示范、没有工具，记忆、
//! 知识库、技能、「关于你」、项目、扩展的几段都没有；不压缩、不起标题、不复述、不跨会话；回合开始不注入事实。请求就是一条
//! 条消息，和原来记忆整理的一次性请求逐字节一样。

use crate::Snapshot;

impl Snapshot {
    /// 照这一份造一份裸的：随核心附带的字照留（回合没走完的几句、驱动的占位这些照样用得上），别的都去掉。
    #[must_use]
    pub fn bare(mut self) -> Snapshot {
        self.bare = true;
        self.persona = None;
        self.system.clear();
        self.demos.clear();
        self.tools.clear();
        self.compaction = None;
        self.jobs = None;
        self.recap = None;
        self.title = None;
        self.peers = None;
        self.memory = None;
        self.reminder = None;
        self.persona_digest = None;
        self.preset = None;
        self.group = None;
        self.foreground = false;
        self.knowledge = None;
        self.skills = None;
        self.profile = None;
        self.project = None;
        self.appended.clear();
        self
    }
}

#[cfg(test)]
mod tests;
