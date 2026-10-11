//! 私聊里带的东西的记号那一行（施工 O-35，`docs/blueprint/kernel/request.md`「群里的一行」私聊那一条）：O-35 起造的会话（快照里
//! 有 `core.private_media`），私聊里带了东西（`venue.media` 不空）的那条，正文后面单独一个文本块
//! `[msg=编号] [image #1: 834 KB] [file #2: 排班.pdf, 1.2 MB]`：她照 `msg=`、`#n` 调 `fetch_media`。
//!
//! 记号和群里那一行同一个写法（`group::markers`：大小、不止一样的从 1 数标 `#n`、语音接那一句），`msg=` 照群里那一行转义。
//! 已经当原图交进来的（`shown`）不写，免得她看到两遍、又去取一张在眼前的；编号照旧连着数，和 `fetch_media` 的 `index`
//! 对得上。全是交了原图的，这一行整行不写。`[msg=…]` 和群里一样总是写，不看 `show_ids`（它只管发的人的平台身份，私聊这一行
//! 没有发的人）。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::VenueMessage;
use miyu_kernel::template::escape;

use crate::group::markers;
use crate::texts::PrivateMedia;

/// 私聊的一条 `venue` 的记号那一行：没交原图的那几样都没有的（没带东西、全交了原图）是没有。
pub(crate) fn line(venue: &VenueMessage, texts: &PrivateMedia) -> Option<Block> {
    let marks: Vec<String> = markers(&venue.media, Some(&texts.voice))
        .into_iter()
        .zip(&venue.media)
        .filter(|(_, item)| !item.shown)
        .map(|(mark, _)| mark)
        .collect();
    (!marks.is_empty()).then(|| {
        Block::Text(Text {
            text: format!("[msg={}] {}", escape(&venue.msg), marks.join(" ")),
        })
    })
}

#[cfg(test)]
mod tests;
