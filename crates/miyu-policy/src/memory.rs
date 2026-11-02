//! 记忆的范围（施工 R-3 下，`docs/blueprint/memory.md`「范围」，`docs/designs/17-记忆.md` L2、L3）：会话开局时定下、记进策略
//! 快照，之后不改。只有两种：跟着人格（默认）、不记；「只在这个会话里」施工 FH-1 去掉了（设计 55 第六节第 3 条，2026-11-02
//! 项目主人定：记忆就是为了跨会话），以前的快照里写着 `session` 的照认不出的算。

use std::fmt;

use crate::Snapshot;

/// 一个会话的记忆的范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    /// 跟着人格：用这个人格的会话都想得起来（默认）。
    Persona,
    /// 不召回、也不记：`miyu ask --no-memory`，子代理（17 第二节）。
    Off,
}

impl MemoryScope {
    /// 写法：`persona`、`off`。
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryScope::Persona => "persona",
            MemoryScope::Off => "off",
        }
    }

    /// 读写法；认不出的是 `None`。
    pub fn parse(text: &str) -> Option<MemoryScope> {
        match text {
            "persona" => Some(MemoryScope::Persona),
            "off" => Some(MemoryScope::Off),
            _ => None,
        }
    }
}

impl fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Snapshot {
    /// 记下记忆的范围。
    #[must_use]
    pub fn with_memory(mut self, scope: MemoryScope) -> Snapshot {
        self.memory = Some(scope.as_str().to_string());
        self
    }

    /// 这个会话的记忆的范围：没有的（以前造的）跟着人格；认不出的不记也不召回，不往不知道的地方写。
    pub fn memory_scope(&self) -> MemoryScope {
        match &self.memory {
            None => MemoryScope::Persona,
            Some(text) => MemoryScope::parse(text).unwrap_or(MemoryScope::Off),
        }
    }
}

#[cfg(test)]
mod tests;
