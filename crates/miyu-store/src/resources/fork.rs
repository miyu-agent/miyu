//! fork 子代理要的两句（施工 A-F 中，`docs/blueprint/agents.md`「fork 子代理」）：都只在派 fork 子代理时读，不进快照（子会话
//! 用父会话那一份快照）。

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 子会话给派它的那一步每个调用记的占位结果：`core/tool-results/forked.txt` 的原文（施工 A-F 上）。
    ///
    /// # Errors
    ///
    /// 读不了这份文件，写明是哪一份。
    pub fn fork_placeholder(&self) -> Result<String, SourceError> {
        self.read(&["core", "tool-results", "forked.txt"])
    }

    /// 交代前面的外壳：`core/jobs/fork-directive.txt` 的原文，接一个空行、再接她写的交代，作为父会话发来的话送进子会话。
    ///
    /// # Errors
    ///
    /// 读不了这份文件，写明是哪一份。
    pub fn fork_directive(&self) -> Result<String, SourceError> {
        self.read(&["core", "jobs", "fork-directive.txt"])
    }
}
