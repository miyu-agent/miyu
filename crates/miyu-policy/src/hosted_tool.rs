//! 供应商侧工具写成的字的四份（施工 HT-2，`docs/blueprint/drivers/openai-chat.md` 第 4 条）：随核心附带的字，存进策略快照的
//! `core.drivers.hosted_tool`；以前造的快照里没有，读成没有，供应商侧工具不写进请求。

use miyu_drivers::HostedToolSources;
use serde::{Deserialize, Serialize};

/// 四份，每一格是 `resources/core/drivers/` 下同名（下划线换成 `-`）文件的原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedToolTexts {
    /// 有结果的（`hosted-tool.txt`）。
    pub hosted_tool: String,
    /// 结果是失败的（`hosted-tool-failed.txt`）。
    pub hosted_tool_failed: String,
    /// 没结果的（`hosted-tool-open.txt`）。
    pub hosted_tool_open: String,
    /// 结果截过的那一句（`hosted-tool-cut.txt`）。
    pub hosted_tool_cut: String,
}

impl HostedToolTexts {
    /// 交给驱动的原文。
    pub(crate) fn sources(&self) -> HostedToolSources<'_> {
        HostedToolSources {
            hosted_tool: &self.hosted_tool,
            hosted_tool_failed: &self.hosted_tool_failed,
            hosted_tool_open: &self.hosted_tool_open,
            hosted_tool_cut: &self.hosted_tool_cut,
        }
    }
}
