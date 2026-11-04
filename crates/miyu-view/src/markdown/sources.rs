//! 条目里没有、导出要的原文（调研第四节第 3 条「照序号回事件里取」）：读日志的一方边喂投影边照事件记下，不另读一遍。
//!
//! - 工具结果切过的（条目只带前面 100 行、8 KiB，`output_cut`）：她当时看到的全文，同 `view.detail` 的 `output`；没切过的
//!   条目里已经整份有了，不记。
//! - 压缩的摘要：照压缩那一条的编号（`c<替代到的那一条>`）记，同 `view.detail` 的 `summary`；清空的没有摘要，不记。

use std::collections::BTreeMap;

use miyu_kernel::block::Block as Content;
use miyu_kernel::event::{Body, CompactTrigger, Event};
use miyu_kernel::id::CallId;

use crate::entry::EntryId;
use crate::projector::output_head;

/// 条目里没有的原文。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// 切过的工具结果的全文：调用编号到她看到的字。
    pub outputs: BTreeMap<CallId, String>,
    /// 压缩的摘要：压缩那一条的编号到摘要原文。
    pub summaries: BTreeMap<EntryId, String>,
}

impl Sources {
    /// 看一条落了盘的事件，要的记下。
    pub fn see(&mut self, event: &Event) {
        match &event.body {
            Body::ToolResult(result) => {
                if let (Some(_), true) = output_head(&result.blocks) {
                    let whole = result
                        .blocks
                        .iter()
                        .filter_map(|block| match block {
                            Content::Text(text) => Some(text.text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<&str>>()
                        .join("\n\n");
                    self.outputs.insert(result.call_id, whole);
                }
            }
            Body::ContextCompacted(compacted)
                if compacted.trigger != Some(CompactTrigger::Clear) =>
            {
                self.summaries.insert(
                    EntryId::compaction(compacted.upto),
                    compacted.summary.clone(),
                );
            }
            _ => {}
        }
    }
}
