//! 看得了的附件开头带上路径的两句（施工 3-9 六补，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：随核心附带的字，存进
//! 策略快照的 `core.drivers.open_path`；以前造的快照里没有，读成没有，照旧只带名字。

use miyu_drivers::OpenPathSources;
use serde::{Deserialize, Serialize};

/// 两句，每一格是 `resources/core/drivers/` 下同名（下划线换成 `-`）文件的原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPathTexts {
    /// 能看图时图片前面，带名字和路径（`image-open-path.txt`）。
    pub image_open_path: String,
    /// 照字放进消息的文本文件开头，带名字和路径（`file-open-path.txt`）。
    pub file_open_path: String,
}

impl OpenPathTexts {
    /// 交给驱动的原文。
    pub(crate) fn sources(&self) -> OpenPathSources<'_> {
        OpenPathSources {
            image_open_path: &self.image_open_path,
            file_open_path: &self.file_open_path,
        }
    }
}
