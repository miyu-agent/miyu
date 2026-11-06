//! 接在整份 messages 最末尾的那一句（施工 RT-1，[`Request::tail`]，core1 11-07 照推荐定）：内核放在请求的一格里，不进 messages、
//! 不进指纹；驱动编码前照统一的写法接上，三家照各自的规矩编：
//!
//! - 最后一条是 user 的，并进那一条做最后一块（openai-chat、responses 全是字的照现成的拼法接在后面，anthropic 多一个 text 块；
//!   anthropic 的 tool 结果本来就在线上的 user 里，后面跟的 user 照旧并成一条）；
//! - 最后一条是 tool 结果（或者别的）的，另起一条 user，只有这一块。
//!
//! 整份编的和接着用留着的历史编的（[`crate::Memo`]）走同一个入口，发出去的字节一样。接上的那一条和平常的消息一样留进备忘录，
//! 下一次那一条没有它了，照「从第一条不一样的起重编」重编那一条：前面的一个字节都不动。

use std::borrow::Cow;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::{Message, Request};

/// 照 `request` 接上那一句交回编码用的请求，和接上了没有：没有、空的、`skip`（openai-chat 前缀续写的那一次，core1 11-07 定）
/// 的照原样借着用。
pub(crate) fn tailed(request: &Request, skip: bool) -> (Cow<'_, Request>, bool) {
    let Some(tail) = request
        .tail
        .as_ref()
        .filter(|tail| !tail.is_empty() && !skip)
    else {
        return (Cow::Borrowed(request), false);
    };
    let mut owned = request.clone();
    owned.tail = None;
    let block = Block::Text(Text { text: tail.clone() });
    match owned.messages.last_mut() {
        Some(Message::User { blocks }) => blocks.push(block),
        _ => owned.messages.push(Message::User {
            blocks: vec![block],
        }),
    }
    (Cow::Owned(owned), true)
}

#[cfg(test)]
mod tests;
