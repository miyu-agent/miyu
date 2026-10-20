//! 整理那一批的正文边流边交（施工 D-1 下）：会话推来的 `model.delta` 里只挑正文那几块的字，同一次性入口的 `call_watching`
//! （R-21 照它边合边推）。一轮里换了一次请求（出错重试、换端点）的，块又从第 0 块开头：先交 [`Watched::Restart`]，之前交的作废。

use std::collections::BTreeMap;

use miyu_kernel::accumulate::Kind;
use miyu_kernel::event::{ModelDelta, Piece};
use miyu_session::Watched;

/// 一批的增量看到哪了。
#[derive(Debug, Default)]
pub(super) struct Stream {
    /// 这一次请求里开了的块：第几块，是不是正文。
    blocks: BTreeMap<usize, bool>,
    /// 这一批交过正文了。
    told: bool,
}

impl Stream {
    /// 看一段增量，正文的字交给 `watch`。
    pub(super) fn see(&mut self, delta: &ModelDelta, watch: &mut (dyn FnMut(Watched<'_>) + Send)) {
        match &delta.piece {
            Piece::Start(kind) => {
                // 又从第 0 块开头：换了一次请求。
                if delta.index == 0 && !self.blocks.is_empty() {
                    self.blocks.clear();
                    if self.told {
                        self.told = false;
                        watch(Watched::Restart);
                    }
                }
                self.blocks.insert(delta.index, matches!(kind, Kind::Text));
            }
            Piece::Text(text)
                if self.blocks.get(&delta.index) == Some(&true) && !text.is_empty() =>
            {
                self.told = true;
                watch(Watched::Text(text));
            }
            Piece::Text(_) | Piece::End => {}
        }
    }
}

#[cfg(test)]
mod tests;
