//! 合并转发那几行的字（施工 O-34，`docs/blueprint/kernel/request.md`「合并转发」）：随核心附带，造会话时冻结在策略快照的
//! `core.forward`。放在 `core` 这一层、不放进群会话的 `group`：私聊会话的快照没有 `group`，私聊里的转发也要写（2026-10-11
//! 主会话、核心定）。以前造的快照里没有，读成没有：那些会话收到转发也不写这几行，和 O-34 以前一样只看到正文里的占位。

use miyu_assemble::ForwardTexts as Rendered;
use miyu_kernel::template::Template;
use serde::{Deserialize, Serialize};

use crate::snapshot::{BuildError, Snapshot};

/// 合并转发那几行的三句（`core/venues/forward/` 下同名的 `.txt`），原文，行尾的换行也算。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardTexts {
    /// 一条消息只有一个转发时的头一行（`label.txt`）。
    pub label: String,
    /// 不止一个时每个的头一行（`numbered.txt`）：字段 `number` 是第几个，从 1 数。
    pub numbered: String,
    /// 没放进来的条数那一行（`more.txt`）：字段 `count`。
    pub more: String,
}

impl ForwardTexts {
    /// 组装器要的：两份模板照字段试换过，三句都去掉行尾的空白。
    ///
    /// # Errors
    ///
    /// 模板写错了、字段不对：快照坏了。
    pub fn texts(&self) -> Result<Rendered, BuildError> {
        let template = |text: &str, field: &str| {
            let broken = |error| BuildError::Texts {
                which: "forward",
                error,
            };
            let template = Template::parse(text.trim_end()).map_err(broken)?;
            template
                .render(&[(field, "1")].into_iter().collect())
                .map_err(broken)?;
            Ok::<_, BuildError>(template)
        };
        Ok(Rendered {
            label: self.label.trim_end().to_string(),
            numbered: template(&self.numbered, "number")?,
            more: template(&self.more, "count")?,
        })
    }
}

impl Snapshot {
    /// 交给组装器的转发那几行的字：快照里有的才有。
    pub(crate) fn forward(&self) -> Result<Option<Rendered>, BuildError> {
        self.core
            .forward
            .as_ref()
            .map(ForwardTexts::texts)
            .transpose()
    }
}

#[cfg(test)]
mod tests;
