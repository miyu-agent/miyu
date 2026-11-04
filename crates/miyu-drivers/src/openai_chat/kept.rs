//! openai-chat 留着的编好的历史（施工 V-42，[`crate::Memo`]）。
//!
//! 一条消息留一截：它自己写成的那一条线上的消息（[`messages::Writer::one`]）；tool 消息另留挪出来的图片、PDF（线上的结构，
//! 没编）。每次照旧现排、现编的：
//!
//! - system：照字比，一样的用留着的那一截；
//! - 一串 tool 后面挪出来的图片另起的那条 user：拿留着的几段照 [`messages::Writer::flush`] 现拼（标签和那一句说明照字拼在一起）；
//! - 接着写：最后那条 user 不发，半截那条 assistant 现编、加上字段（留着的是不加字段的那一份）；
//! - 工具面：照工具面原样比，一样的用留着的那一截；发不发照旧看有没有工具、历史里有没有工具调用；
//! - 头尾（模型名、输出上限、温度、思考强度）每次照这一次的写。

use miyu_kernel::request::{Message, Request, ToolSpec};

use super::messages::{self, Writer};
use super::wire::{self, Part, Wire};
use super::{Compat, EncodeError, Encoded, FAMILY, continuing, head, path, rest};
use crate::fragments::{Builder, piece};
use crate::memo::{Setup, Slot};
use crate::{BlobBytes, Call, DriverTexts, Fragments, Memo};

/// openai-chat 留着的。
#[derive(Debug, Default)]
pub(crate) struct Kept {
    /// 编好的每一条，和 [`crate::memo::Core`] 里留着的消息一一对应。
    pub(crate) entries: Vec<Entry>,
    /// system 的原文和它写成的那一条。
    system: Option<(String, Fragments)>,
    /// 工具面原样的一份和它写成的那一截（`[…]`）。
    tools: Option<(Vec<ToolSpec>, Fragments)>,
}

/// 编好的一条。
#[derive(Debug)]
pub(crate) struct Entry {
    /// 这条消息写成的那一条线上的消息。
    wire: Fragments,
    /// tool 消息里挪出来的图片、PDF。
    moved: Vec<Part>,
}

/// 编码（[`super::fragments_memo`]）。
pub(super) fn fragments(
    request: &Request,
    call: &Call,
    compat: &Compat,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
    memo: &mut Memo,
) -> Result<Encoded<Fragments>, EncodeError> {
    let continuing = continuing(compat, request).map(|(field, _)| field);
    // 接着写的：最后那条 user 不发，也不留。
    let messages = match (continuing, request.messages.split_last()) {
        (Some(_), Some((Message::User { .. }, earlier))) => earlier,
        _ => &request.messages[..],
    };
    let setup = Setup {
        family: FAMILY,
        compat: Some(compat),
        texts,
        inputs: call.inputs,
        described: &request.described,
    };
    let reused = memo.reuse(&setup, messages, call, blobs, messages::provider_id);
    let Memo { core, slot } = memo;
    if !matches!(slot, Slot::Chat(_)) {
        *slot = Slot::Chat(Kept::default());
    }
    let Slot::Chat(kept) = slot else {
        unreachable!("刚放进去的就是 openai-chat 的")
    };
    {
        let writer = Writer::new(request, call, compat, texts, blobs, core.ids());
        for message in &messages[reused..] {
            let mut moved = Vec::new();
            let wire = writer.one(message, &mut moved)?;
            kept.entries.push(Entry {
                wire: piece(&wire),
                moved,
            });
        }
    }
    core.extend(&messages[reused..], call, blobs);
    core.settle(reused, messages::provider_id);
    let writer = Writer::new(request, call, compat, texts, blobs, core.ids());

    let mut body = head(call);
    body.reserve(kept.entries.iter().map(|entry| entry.wire.text_len()).sum());
    let mut ranges = Vec::with_capacity(messages.len() + 1);
    let mut put = |body: &mut Builder, write: &dyn Fn(&mut Builder)| {
        if !ranges.is_empty() {
            body.push(b',');
        }
        let start = body.len();
        write(body);
        ranges.push(start..body.len());
    };
    if !request.system.is_empty() {
        if kept
            .system
            .as_ref()
            .is_none_or(|(system, _)| *system != request.system)
        {
            let wire = Wire::System {
                content: request.system.clone(),
            };
            kept.system = Some((request.system.clone(), piece(&wire)));
        }
        if let Some((_, system)) = &kept.system {
            put(&mut body, &|body| body.append(system));
        }
    }
    let mut moved = Vec::new();
    let last = messages.len().checked_sub(1);
    for (index, (message, entry)) in messages.iter().zip(&kept.entries).enumerate() {
        if !matches!(message, Message::Tool { .. }) {
            flush(&writer, &mut moved, &mut body, &mut put);
        }
        match (continuing, message) {
            // 接着写的半截那一条：现编、加上字段。
            (Some(field), Message::Assistant { .. }) if Some(index) == last => {
                let mut wire = writer.one(message, &mut Vec::new())?;
                messages::continue_at(&mut wire, field);
                put(&mut body, &|body| body.json(&wire));
            }
            _ => put(&mut body, &|body| body.append(&entry.wire)),
        }
        moved.extend(entry.moved.iter().cloned());
    }
    flush(&writer, &mut moved, &mut body, &mut put);
    body.push(b']');
    if wire::listed(request) {
        if kept
            .tools
            .as_ref()
            .is_none_or(|(tools, _)| *tools != request.tools)
        {
            let tools = wire::tools(request).unwrap_or_default();
            kept.tools = Some((request.tools.clone(), piece(&tools)));
        }
        if let Some((_, tools)) = &kept.tools {
            body.extend_from_slice(b",\"tools\":");
            body.append(tools);
        }
    }
    rest(&mut body, call, compat);
    Ok(Encoded {
        body: body.finish(),
        messages: ranges,
        path: path(compat, request),
    })
}

/// 挪出来的图片、PDF 照 [`Writer::flush`] 拼成一条 user 写上；没有就什么都不写。
fn flush(
    writer: &Writer<'_>,
    moved: &mut Vec<Part>,
    body: &mut Builder,
    put: &mut impl FnMut(&mut Builder, &dyn Fn(&mut Builder)),
) {
    let mut out = Vec::new();
    writer.flush(moved, &mut out);
    for wire in &out {
        put(body, &|body| body.json(wire));
    }
}
