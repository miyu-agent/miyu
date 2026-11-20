//! 戳一戳那一句（施工 VP-1，`internal/blueprint/kernel/request.md`「群里的一行」）：带 `venue.poke` 的一条（桥当成戳的人发来的一条
//! 空消息交进来）照造会话时冻下的 `core/venues/poke.txt` 写成 `小林 poked you (a QQ nudge: they double-clicked your avatar)`，
//! 有 `poke_text`（平台上那个人自己设的后缀）的空一格转义接在后面。群里那一行的内容（群聊近况、判官看的那一行同一份写法）、
//! 私聊那一条都照它。
//!
//! 名字照 `venue.name`（没有的写平台身份），平台名照发的人的平台身份的头一截（`qq` 写 `QQ`），都是转义过的；VP-1 以前造的会话
//! 快照里没有那一句，只写记号 `[poke]`（代码写的格式，同 `[image]`）：空着的一条私聊渲染出来没有块，供应商回 400。

use miyu_kernel::event::VenueMessage;
use miyu_kernel::id::ExternalId;
use miyu_kernel::origin::By;
use miyu_kernel::template::{Template, escape};

/// 快照里没有那一句时写的记号。
const MARK: &str = "[poke]";

/// 戳一戳那一句：`venue` 不是戳一戳的没有。`by` 是戳的人，`sentence` 是快照里冻下的那一句（没有的写 [`MARK`]）。
pub(crate) fn said(by: &By, venue: &VenueMessage, sentence: Option<&Template>) -> Option<String> {
    if !venue.poke {
        return None;
    }
    let Some(sentence) = sentence else {
        return Some(MARK.to_string());
    };
    let id = external(by);
    let name = venue
        .name
        .as_deref()
        .or(id.map(ExternalId::as_str))
        .unwrap_or_default();
    let platform = id
        .and_then(|id| id.as_str().split_once(':'))
        .map_or("", |(prefix, _)| platform(prefix));
    let fields = [("name", name), ("platform", platform)]
        .into_iter()
        .collect();
    // 造快照时照这两个字段试换过（`miyu-policy`），换不出来只会是快照坏了：退回记号，不丢这一条。
    let mut text = sentence
        .render(&fields)
        .unwrap_or_else(|_| MARK.to_string());
    if let Some(suffix) = venue
        .poke_text
        .as_deref()
        .map(str::trim)
        .filter(|suffix| !suffix.is_empty())
    {
        text.push(' ');
        text.push_str(&escape(suffix));
    }
    Some(text)
}

/// 发的人的平台身份：群里的人是外部身份，经对应表认出的本人是它对应的那一个；别的没有。
fn external(by: &By) -> Option<&ExternalId> {
    match by {
        By::External(external) => Some(&external.id),
        By::Person(person) => person.via.as_ref(),
        _ => None,
    }
}

/// 平台身份头一截（`qq`）给她看的平台名：认识的几个写惯用的大小写，别的原样（同 `miyu-tool` 记忆那几行的写法）。
fn platform(prefix: &str) -> &str {
    match prefix {
        "qq" => "QQ",
        "wechat" => "WeChat",
        "telegram" => "Telegram",
        "discord" => "Discord",
        other => other,
    }
}

#[cfg(test)]
mod tests;
