//! 整个会话喂投影（施工 SE-1 中下上：导出 Markdown，调研 `docs/reviews/2026-09-30-会话导出调研.md` 第四节第 3 条）：`view.page`
//! 一页最多 50 轮，导出要整个会话。照 `view/read.rs` 的路子边读边喂：日志照先后一块块读（`read_range`），每一块喂完就放掉，
//! 不整份读成事件；喂的是同一台投影、同一套字，条目和 `view.page` 写 `view: true` 从第一条翻到最后一条的一样。
//!
//! 只读地读会话目录，不载入会话：在跑的会话照这一刻已经落盘的整行取（正在写的那半行不算），日志只追加，取到的是一致的一份。
//! 读到的照旧自检：坏了的报出来，不半截地导。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;
use miyu_store::blob::Blobs;
use miyu_store::log::{Flow, OpenError, read_range};
use miyu_view::{Projector, Texts};

use super::project;

/// 喂完整个会话的投影，和喂到的最后一条的序号。
pub(crate) struct Whole {
    /// 喂过每一条的投影：条目照 [`Projector::entries`] 取。
    pub(crate) projector: Projector,
    /// 最后一条落了盘的序号。
    pub(crate) upto: Seq,
}

/// 把会话目录 `dir` 里落了盘的每一条照先后喂进一台新的投影（字照 `texts`，改了多少行照 `blobs`），每一条喂完再交给 `each`
/// 看一眼（导出照它记条目里没有的原文、开头的几样）。碰磁盘。
///
/// # Errors
///
/// 没有这个会话（一条都没有的也算）；日志坏了；读写出错。
pub(crate) fn whole(
    dir: &Path,
    texts: Arc<Texts>,
    blobs: Blobs,
    mut each: impl FnMut(&Event),
) -> Result<Whole, OpenError> {
    let mut projector = project::fresh(texts, blobs);
    let mut upto = None;
    read_range(dir, Seq::FIRST, None, |events| {
        for event in &events {
            projector.event(event);
            each(event);
            upto = Some(event.seq);
        }
        Flow::Go
    })?;
    let upto = upto.ok_or_else(|| OpenError::Missing(dir.to_path_buf()))?;
    Ok(Whole { projector, upto })
}

#[cfg(test)]
mod tests;
