//! 项目的附加提示词（施工 H-3，设计 `38-项目.md` 第四节）：会话在项目里、项目写了附加提示词的，照模板 `core/project.txt` 拼成一块，
//! 接在「关于你」后面。会话挪进挪出项目、项目改了提示词，下一轮换上（会话那一层回合开始时比）。值是本人写的，原文照放。

use crate::Snapshot;
use crate::compose::system;

/// 照模板 `template` 拼一块：附加提示词去掉首尾空白以后放进 `{prompt}`；空的没有。
pub fn block(template: &str, prompt: &str) -> Option<String> {
    let prompt = prompt.trim();
    (!prompt.is_empty()).then(|| template.replace("{prompt}", prompt))
}

impl Snapshot {
    /// 带上项目的附加提示词那一块（施工 H-3）：在 [`Snapshot::with_profile`] 之后、[`Snapshot::with_core_lines`] 之前调；记进
    /// 快照。没有的 system 一字不变。
    #[must_use]
    pub fn with_project(mut self, block: Option<String>) -> Snapshot {
        if let Some(block) = &block {
            self.system = system(&[&self.system, block]);
        }
        self.project = block;
        self
    }
}

#[cfg(test)]
mod tests;
