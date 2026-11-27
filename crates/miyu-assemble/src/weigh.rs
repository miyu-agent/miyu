//! 一条事件进请求以后写成的块（施工 CT-2，`internal/blueprint/compaction.md` 第三条第 2 条）：内核留尾巴、找最大的那一条、截短
//! 重试时照它估（`Assembler::rendered`）。只用来估大小。
//!
//! 群里一句话常常只有几个字，写进请求的那一行却带着钟点、名字、平台身份、编号，引用、@ 那两行。只数正文的话，热闹的群里
//! 尾巴估出来只有实际的四分之一上下，压完还贴着压缩线（2026-11-27 主人的群会话停在 `too_large`）。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;

use crate::group;
use crate::render::{known, spoken};
use crate::texts::Texts;

/// `event` 写成的块：人这边的一条照主请求的写法（群里的一行、私聊带的记号、子代理和别的会话的包装），旁听的一句和别的线替她
/// 发进群里的话照群聊近况里的一行，不进近况的是空的；人用 `!` 跑的命令和别的事件照原样估，没有。
pub(crate) fn rendered(history: &History, event: &Event, texts: &Texts) -> Option<Vec<Block>> {
    match &event.body {
        Body::MessageUser(message) if message.venue.as_ref().is_some_and(|venue| venue.ambient) => {
            Some(overheard(history, event, texts))
        }
        // 和它的 `shell.ran` 拼成一块，内核照两条各自的字估，加起来一样。
        Body::MessageUser(message) if message.shell => None,
        Body::MessageUser(message) => Some(spoken(
            history,
            event,
            message,
            known(&message.blocks),
            texts,
        )),
        Body::VenueDelivered(_) => Some(overheard(history, event, texts)),
        _ => None,
    }
}

/// 群聊近况里的一行，加上行尾的换行；不进近况的（快照里没有近况的字、睡着时收到的、这条线自己发的）是空的。
fn overheard(history: &History, event: &Event, texts: &Texts) -> Vec<Block> {
    group::overheard(history, event, texts)
        .map(|line| vec![Block::Text(Text { text: line + "\n" })])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
