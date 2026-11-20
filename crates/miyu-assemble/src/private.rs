//! 私聊里带的东西的记号那一行（施工 O-35，`internal/blueprint/kernel/request.md`「群里的一行」私聊那一条）：O-35 起造的会话（快照里
//! 有 `core.private_media`），私聊里带了东西（`venue.media` 不空）的那条，正文后面单独一个文本块
//! `[msg=编号] [image #1: 834 KB] [file #2: 排班.pdf, 1.2 MB]`：她照 `msg=`、`#n` 调 `fetch_media`。
//!
//! 记号和群里那一行同一个写法（`group::markers`：大小、不止一样的从 1 数标 `#n`、语音接那一句），`msg=` 照群里那一行转义。
//! 已经当原图交进来的（`shown`）不写，免得她看到两遍、又去取一张在眼前的；编号照旧连着数，和 `fetch_media` 的 `index`
//! 对得上。全是交了原图的，这一行整行不写。`[msg=…]` 和群里一样总是写，不看 `show_ids`（它只管发的人的平台身份，私聊这一行
//! 没有发的人）。
//!
//! 引用（施工 QT-2）：引用了的在最后单独一块写 `reply-to: msg=编号`，带了 `quoted` 的下面缩进 2 格写出那一条，写法同群里那一行
//! （[`tail`]）。没引用的和以前一字不差。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::VenueMessage;
use miyu_kernel::origin::By;
use miyu_kernel::template::escape;

use crate::group::{self, markers};
use crate::texts::{ForwardTexts, PrivateMedia};

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

/// 戳一戳（施工 VP-1，`poke.rs`）：那一句单独一块排在正文的块前面；不是戳一戳的原样交回。`media` 是快照里私聊记号的字，
/// 那一句在它里面，没有的写记号。
pub(crate) fn poked(
    by: &By,
    venue: &VenueMessage,
    mut blocks: Vec<Block>,
    media: Option<&PrivateMedia>,
) -> Vec<Block> {
    let sentence = media.and_then(|media| media.poke.as_ref());
    if let Some(text) = crate::poke::said(by, venue, sentence) {
        blocks.insert(0, Block::Text(Text { text }));
    }
    blocks
}

/// 私聊的一条接在最后的几块：引用那一行（施工 QT-2，同群里那一行的写法、不缩进，带了 `quoted` 的下面写出它）、合并转发那几行
/// （施工 O-34），各自一块；没引用、没转发的没有。引用的内容、转发都要快照里有转发那几句的字（`forward`）才写。
pub(crate) fn tail(
    venue: &VenueMessage,
    forward: Option<&ForwardTexts>,
) -> impl Iterator<Item = Block> {
    let reply = group::reply(venue, "", 0, forward);
    let forwards = forward.and_then(|forward| group::forwarded(&venue.forwards, 0, forward));
    reply
        .into_iter()
        .chain(forwards)
        .map(|text| Block::Text(Text { text }))
}

#[cfg(test)]
mod tests;
