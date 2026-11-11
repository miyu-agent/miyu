//! 只有路径的附件写成的两句（施工 AT-1，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：随核心附带的字，存进策略快照的
//! `core.drivers.path_only`；以前造的快照里没有，读成没有，这种附件只写路径本身。

use miyu_drivers::PathOnlySources;
use serde::{Deserialize, Serialize};

/// 两句，每一格是 `resources/core/drivers/` 下同名（下划线换成 `-`）文件的原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathOnlyTexts {
    /// 没写大小的，带名字和路径（`file-path-only.txt`）。
    pub file_path_only: String,
    /// 写了大小的，带名字、大小和路径（`file-path-only-sized.txt`）。
    pub file_path_only_sized: String,
}

impl PathOnlyTexts {
    /// 交给驱动的原文。
    pub(crate) fn sources(&self) -> PathOnlySources<'_> {
        PathOnlySources {
            file_path_only: &self.file_path_only,
            file_path_only_sized: &self.file_path_only_sized,
        }
    }
}
