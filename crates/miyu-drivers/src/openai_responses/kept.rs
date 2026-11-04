//! openai-responses 留着的编好的历史（施工 V-42，[`crate::Memo`]）。
//!
//! 一条消息留它写成的几项（[`input::Writer::one`]），每项单独编好。这一家的项和前后无关，照先后接上就是。system（写在
//! `instructions`）、工具面照字、照工具面原样比，一样的用留着的那一截；头尾每次照这一次的写。

use miyu_kernel::request::{Request, ToolSpec};

use super::input::{self, Writer};
use super::wire;
use super::{EncodeError, Encoded, FAMILY, PATH, head, rest};
use crate::fragments::piece;
use crate::memo::{Setup, Slot};
use crate::{BlobBytes, Call, DriverTexts, Fragments, Memo};

/// openai-responses 留着的。
#[derive(Debug, Default)]
pub(crate) struct Kept {
    /// 编好的每一条（几项），和 [`crate::memo::Core`] 里留着的消息一一对应。
    pub(crate) entries: Vec<Vec<Fragments>>,
    /// system 的原文和它写成的 JSON 字符串。
    system: Option<(String, Fragments)>,
    /// 工具面原样的一份和它写成的那一截（`[…]`）。
    tools: Option<(Vec<ToolSpec>, Fragments)>,
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
    let reused = memo.reuse(&setup, messages, call, blobs, input::provider_id);
    let Memo { core, slot } = memo;
    if !matches!(slot, Slot::Responses(_)) {
        *slot = Slot::Responses(Kept::default());
    }
    let Slot::Responses(kept) = slot else {
        unreachable!("刚放进去的就是 openai-responses 的")
    };
    {
        let writer = Writer::new(request, call, texts, blobs, core.ids());
        for message in &messages[reused..] {
            let mut items = Vec::new();
            writer.one(message, &mut items)?;
            kept.entries.push(items.iter().map(piece).collect());
        }
    }
    core.extend(&messages[reused..], call, blobs);
    core.settle(reused, input::provider_id);

    let mut body = head(call);
    body.reserve(
        kept.entries
            .iter()
            .flatten()
            .map(|item| item.text_len() + 1)
            .sum(),
    );
    if !request.system.is_empty() {
        if kept
            .system
            .as_ref()
            .is_none_or(|(system, _)| *system != request.system)
        {
            kept.system = Some((request.system.clone(), piece(&request.system)));
        }
        if let Some((_, system)) = &kept.system {
            body.extend_from_slice(b",\"instructions\":");
            body.append(system);
        }
    }
    body.extend_from_slice(b",\"input\":[");
    let mut ranges = Vec::new();
    for item in kept.entries.iter().flatten() {
        if !ranges.is_empty() {
            body.push(b',');
        }
        let start = body.len();
        body.append(item);
        ranges.push(start..body.len());
    }
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
    rest(&mut body, call);
    Ok(Encoded {
        body: body.finish(),
        messages: ranges,
        path: PATH.to_string(),
    })
}
