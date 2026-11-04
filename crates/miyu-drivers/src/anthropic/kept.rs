//! anthropic 留着的编好的历史（施工 V-42，[`crate::Memo`]）。
//!
//! 一条消息留一截：它写成的角色和一块一块（[`messages::Writer::one`]，每块单独编好、不带打点，记着能不能打点）。每次照旧现排的：
//!
//! - 相邻两条角色一样的合成一条、一块都没有的不发（照 [`messages::write`] 的排法）；
//! - 缓存打点（[`marks::place`]）：留着的块不带打点，排好以后照这一次的位置补上；
//! - system、工具面：照字、照工具面原样比，一样的用留着的那一截（不带打点），打点照这一次的补；
//! - 头尾（模型名、输出上限、温度、思考强度）每次照这一次的写。

use miyu_kernel::request::{Request, ToolSpec};

use super::marks::{self, Shape};
use super::messages::{self, Writer};
use super::wire;
use super::{EncodeError, Encoded, FAMILY, head, mark, rest};
use crate::fragments::{Builder, piece};
use crate::memo::{Setup, Slot};
use crate::{BlobBytes, Call, DriverTexts, Fragments, Memo};

/// anthropic 留着的。
#[derive(Debug, Default)]
pub(crate) struct Kept {
    /// 编好的每一条，和 [`crate::memo::Core`] 里留着的消息一一对应。
    pub(crate) entries: Vec<Entry>,
    /// system 的原文和它写成的那一块（不带打点）。
    system: Option<(String, Fragments)>,
    /// 工具面原样的一份和每一件写成的那一块（不带打点）。
    tools: Option<(Vec<ToolSpec>, Vec<Fragments>)>,
}

/// 编好的一条：角色，和一块一块（能不能打点）。
#[derive(Debug)]
pub(crate) struct Entry {
    role: &'static str,
    parts: Vec<(Fragments, bool)>,
}

/// 现排的线上的消息。
struct Layout {
    /// 合并好的每一条。
    messages: Vec<Merged>,
    /// 前 `stable` 条消息写出的最后一块。
    stable_end: Option<(usize, usize)>,
}

/// 合并好的一条：角色、从第几条消息开始、各块是第几条消息的第几块、能不能打点。
struct Merged {
    role: &'static str,
    first: usize,
    blocks: Vec<(usize, usize)>,
    markable: Vec<bool>,
}

impl Shape for Layout {
    fn len(&self) -> usize {
        self.messages.len()
    }

    fn role(&self, m: usize) -> &'static str {
        self.messages[m].role
    }

    fn first(&self, m: usize) -> usize {
        self.messages[m].first
    }

    fn blocks(&self, m: usize) -> usize {
        self.messages[m].blocks.len()
    }

    fn markable(&self, m: usize, b: usize) -> bool {
        self.messages[m].markable[b]
    }

    fn stable_end(&self) -> Option<(usize, usize)> {
        self.stable_end
    }
}

/// 编码（[`super::fragments_memo`]）。
pub(super) fn fragments(
    request: &Request,
    call: &Call,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
    memo: &mut Memo,
) -> Result<Encoded<Fragments>, EncodeError> {
    let messages = &request.messages[..];
    let setup = Setup {
        family: FAMILY,
        compat: None,
        texts,
        inputs: call.inputs,
        described: &request.described,
    };
    let reused = memo.reuse(&setup, messages, call, blobs, messages::provider_id);
    let Memo { core, slot } = memo;
    if !matches!(slot, Slot::Anthropic(_)) {
        *slot = Slot::Anthropic(Kept::default());
    }
    let Slot::Anthropic(kept) = slot else {
        unreachable!("刚放进去的就是 anthropic 的")
    };
    {
        let writer = Writer::new(request, call, texts, blobs, core.ids());
        for message in &messages[reused..] {
            let (role, parts) = writer.one(message)?;
            kept.entries.push(Entry {
                role,
                parts: parts
                    .iter()
                    .map(|part| (piece(part), part.markable()))
                    .collect(),
            });
        }
    }
    core.extend(&messages[reused..], call, blobs);
    core.settle(reused, messages::provider_id);

    // 照 `messages::write` 的排法合并：相邻两条角色一样的合成一条，一块都没有的不发。
    let mut layout = Layout {
        messages: Vec::new(),
        stable_end: None,
    };
    for (index, entry) in kept.entries.iter().enumerate() {
        if !entry.parts.is_empty() {
            let blocks = (0..entry.parts.len()).map(|b| (index, b));
            let markable = entry.parts.iter().map(|(_, markable)| *markable);
            match layout.messages.last_mut() {
                Some(last) if last.role == entry.role => {
                    last.blocks.extend(blocks);
                    last.markable.extend(markable);
                }
                _ => layout.messages.push(Merged {
                    role: entry.role,
                    first: index,
                    blocks: blocks.collect(),
                    markable: markable.collect(),
                }),
            }
        }
        if index + 1 == request.stable
            && let Some(last) = layout.messages.last()
        {
            layout.stable_end = Some((layout.messages.len() - 1, last.blocks.len() - 1));
        }
    }

    let listed = wire::listed(request);
    if listed
        && kept
            .tools
            .as_ref()
            .is_none_or(|(tools, _)| *tools != request.tools)
    {
        let tools = wire::tools(request).unwrap_or_default();
        kept.tools = Some((request.tools.clone(), tools.iter().map(piece).collect()));
    }
    let marks = marks::place(request, listed.then_some(request.tools.len()), &layout);

    let mut body = head(call);
    body.reserve(
        kept.entries
            .iter()
            .flat_map(|entry| &entry.parts)
            .map(|(part, _)| part.text_len() + 1)
            .sum(),
    );
    if !request.system.is_empty() {
        if kept
            .system
            .as_ref()
            .is_none_or(|(system, _)| *system != request.system)
        {
            let part = wire::Part::text(&request.system);
            kept.system = Some((request.system.clone(), piece(&part)));
        }
        if let Some((_, system)) = &kept.system {
            body.extend_from_slice(b",\"system\":[");
            block(&mut body, system, marks.system);
            body.push(b']');
        }
    }
    if listed && let Some((_, tools)) = &kept.tools {
        body.extend_from_slice(b",\"tools\":[");
        for (index, tool) in tools.iter().enumerate() {
            if index > 0 {
                body.push(b',');
            }
            block(&mut body, tool, marks.tool == Some(index));
        }
        body.push(b']');
    }
    body.extend_from_slice(b",\"messages\":[");
    let mut ranges = Vec::with_capacity(layout.messages.len());
    for (m, merged) in layout.messages.iter().enumerate() {
        if m > 0 {
            body.push(b',');
        }
        let start = body.len();
        body.extend_from_slice(b"{\"role\":");
        body.json(merged.role);
        body.extend_from_slice(b",\"content\":[");
        for (b, (index, part)) in merged.blocks.iter().enumerate() {
            if b > 0 {
                body.push(b',');
            }
            let (part, _) = &kept.entries[*index].parts[*part];
            block(&mut body, part, marks.messages.contains(&(m, b)));
        }
        body.extend_from_slice(b"]}");
        ranges.push(start..body.len());
    }
    rest(&mut body, call);
    Ok(Encoded {
        body: body.finish(),
        messages: ranges,
        path: super::PATH.to_string(),
    })
}

/// 接上留着的一块；打了点的补上打点（同 `anthropic.rs` 的 `block`）。
fn block(body: &mut Builder, part: &Fragments, marked: bool) {
    body.append(part);
    if marked {
        mark(body);
    }
}
