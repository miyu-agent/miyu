//! 项目指令那一块的几份（施工 P-14，`docs/blueprint/project-docs.md`），都在 `core/project-docs/` 下。

use super::{ResourceRoot, SourceError};

/// 项目指令那一块的字（[`ResourceRoot::project_doc_texts`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDocTexts {
    /// 一份文件的外壳头（`open.txt`）：字段 `path`。
    pub open: String,
    /// 外壳尾（`close.txt`）。
    pub close: String,
    /// 文件改了、换一份时头一句（`replaced.txt`）：字段 `root`。
    pub replaced: String,
    /// 超了总上限、截断了的那一句（`cut.txt`）：字段 `limit`。
    pub cut: String,
}

impl ResourceRoot {
    /// 项目指令那一块的字（施工 P-14，`26-提示词.md` 第十节）：回合开始读。
    ///
    /// # Errors
    ///
    /// 读不了其中一份，写明是哪一份。
    pub fn project_doc_texts(&self) -> Result<ProjectDocTexts, SourceError> {
        let read = |name: &str| self.read(&["core", "project-docs", name]);
        Ok(ProjectDocTexts {
            open: read("open.txt")?,
            close: read("close.txt")?,
            replaced: read("replaced.txt")?,
            cut: read("cut.txt")?,
        })
    }
}
