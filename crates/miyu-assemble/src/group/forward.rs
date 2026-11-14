//! 合并转发那几行（施工 O-34，`docs/blueprint/kernel/request.md`「合并转发」）：一条场所消息的 `venue.forwards` 写成缩进的
//! 几行，群里的接在那一行下面（`group.rs` 的 `record`），私聊的接在正文后面（`render.rs` 的 `spoken`）。
//!
//! 只有一个转发的头一行写 `label`，不止一个的每个写 `numbered`（第几个从 1 数）；下面每条一行 `名字: 正文 [记号]`，套在里面的
//! 转发接在那一条下面、再缩进一层；`more` 有的最后写一行 `more`。缩进只由这里加，名字、正文、记号里的名字照模板的规矩转义成
//! 一行：伪造不出不缩进的记录行。不拿正文里 `[forward]` 的个数去对转发的个数（人也打得出这几个字），全部照先后写。带的东西
//! 桥记了编号的（施工 O-61）标 `#编号`，照它用 `fetch_media` 取；没记的不标；语音不接那一句。
//!
//! 引用的那一条的内容（施工 QT-1，`venue.quoted`）同转发里的一条：一行 `名字: 正文 [记号]`，套在里面的转发接在下面（[`quoted`]）。

use std::collections::BTreeMap;

use miyu_kernel::event::{Forward, ForwardNode};
use miyu_kernel::template::escape;

use super::marker;
use crate::texts::ForwardTexts;

/// 一层多缩进几格。
const STEP: usize = 2;

/// `forwards` 写成的几行，头一行缩进 `indent` 格，行和行之间一个换行、最后不带换行；没有转发的没有。
pub(crate) fn block(forwards: &[Forward], indent: usize, texts: &ForwardTexts) -> Option<String> {
    if forwards.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    push(&mut lines, forwards, indent, texts);
    Some(lines.join("\n"))
}

/// 把一层的几个转发写进 `lines`。
fn push(lines: &mut Vec<String>, forwards: &[Forward], indent: usize, texts: &ForwardTexts) {
    let pad = " ".repeat(indent);
    let inner = " ".repeat(indent + STEP);
    for (forward, number) in forwards.iter().zip(1..) {
        let head = if forwards.len() == 1 {
            texts.label.clone()
        } else {
            let number = number.to_string();
            let fields = BTreeMap::from([("number", number.as_str())]);
            // 造快照时试换过，这里换不出只会是造的时候没查到的 bug，照空的写。
            texts.numbered.render(&fields).unwrap_or_default()
        };
        lines.push(format!("{pad}{head}"));
        for one in &forward.nodes {
            node(lines, one, indent + STEP, texts);
        }
        if let Some(more) = forward.more.filter(|more| *more > 0) {
            let count = more.to_string();
            let fields = BTreeMap::from([("count", count.as_str())]);
            lines.push(format!(
                "{inner}{}",
                texts.more.render(&fields).unwrap_or_default()
            ));
        }
    }
}

/// 引用的那一条（施工 QT-1）写成的几行：头一行缩进 `indent` 格，套在里面的转发接在下面、再缩进一层；行和行之间一个换行、
/// 最后不带换行。
pub(crate) fn quoted(quoted: &ForwardNode, indent: usize, texts: &ForwardTexts) -> String {
    let mut lines = Vec::new();
    node(&mut lines, quoted, indent, texts);
    lines.join("\n")
}

/// 一条写进 `lines`：`名字: 正文 [记号]` 缩进 `indent` 格，正文和记号都没有的只写 `名字:`；套在里面的转发接在下面，头一行再缩进
/// 一层。
fn node(lines: &mut Vec<String>, node: &ForwardNode, indent: usize, texts: &ForwardTexts) {
    let pad = " ".repeat(indent);
    let mut parts = Vec::new();
    let text = node.text.trim();
    if !text.is_empty() {
        parts.push(escape(text));
    }
    // 编号照桥记的写（施工 O-61），核心不自己数。
    parts.extend(node.media.iter().map(|item| {
        let number = item
            .index
            .and_then(|index| usize::try_from(index.get()).ok());
        marker(item, number)
    }));
    let name = escape(&node.name);
    if parts.is_empty() {
        lines.push(format!("{pad}{name}:"));
    } else {
        lines.push(format!("{pad}{name}: {}", parts.join(" ")));
    }
    push(lines, &node.forwards, indent + STEP, texts);
}

#[cfg(test)]
mod tests;
