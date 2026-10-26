//! 记忆用的几张表的原文（`core/memory/`，施工 R-6 上起；施工 J-1 上从 `resources.rs` 挪来：那一份到了 500 行）。

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 常见的 key 写法的原文（`core/memory/secrets.toml`，施工 R-6 上）：抽取发出去以前、记下以前照它遮 key。怎么读由
    /// `miyu_recall::redact` 定。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn memory_secrets(&self) -> Result<String, SourceError> {
        self.read(&["core", "memory", "secrets.toml"])
    }

    /// 联想的门槛表的原文（`core/memory/recall.toml`，施工 R-8）：照模型，相似度不到门槛的不带。怎么读由
    /// `miyu_recall::associate` 定。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn memory_recall(&self) -> Result<String, SourceError> {
        self.read(&["core", "memory", "recall.toml"])
    }

    /// 记忆的一张下限表的原文（`core/memory/<name>.toml`）：`search` 是主动搜时只有关键词找到的那几条的下限（施工 R-14），
    /// `turns` 是搜以前的对话时只照意思找到的那几段的（施工 R-20），`near` 是搜记下的时只照意思找到的那几条的（施工 R-24）；
    /// 写法同 [`ResourceRoot::memory_recall`]。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn memory_table(&self, name: &str) -> Result<String, SourceError> {
        self.read(&["core", "memory", &format!("{name}.toml")])
    }
}
