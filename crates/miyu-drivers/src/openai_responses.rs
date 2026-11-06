//! OpenAI 的 Responses 接口（`docs/blueprint/drivers/openai-responses.md`，施工 8-13）：OpenAI 官方、opencode Zen 上的 GPT
//! 说这一种。
//!
//! 不存对话：每次请求带全部历史、`"store":false`，不用 `previous_response_id`，对话的真相只在我们的日志里。统一的请求照那一页
//! 写成一个 JSON：顶层照 `model`、`instructions`、`input`、`tools`、`store`、`stream`、`max_output_tokens`、思考强度的先后，
//! `input` 里一项一项照 `openai_responses/input.rs` 写，同样的输入字节一定一样。缓存是自动的、按前缀，不打点。响应是 SSE
//! 流，[`Decoder`] 解成内核的四种增量。列模型和 openai-chat 一样（[`crate::openai_chat::parse_models`]）。

mod decode;
mod effort;
mod input;
mod kept;
mod usage;
mod wire;

pub use decode::Decoder;
pub(crate) use kept::Kept;

use std::collections::BTreeSet;

use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;

pub use crate::{EncodeError, Encoded};

use crate::fragments::Builder;
use crate::{BlobBytes, Call, DriverTexts, Fragments, Memo, media};

/// 驱动家族：私有数据里写的是它的，才是这个驱动的（`03-事件模型.md` 第九节）。
pub const FAMILY: &str = "openai-responses";

/// 请求发到供应商地址后面的这一截。
pub const PATH: &str = "/responses";

/// 编码：顶层照 `model`、`instructions`、`input`、`tools`、`"store":false`、`"stream":true`、`max_output_tokens`、思考强度的
/// 先后写，别的字段一概不发。带着接着写记号的照原样发（这一家没有前缀续写），发到 [`PATH`]。请求体整份写成字节
/// （[`fragments`] 拼起来）。
///
/// # Errors
///
/// 要用的 blob 执行器没交进来：先照 [`blobs_needed`] 取。
pub fn encode(
    request: &Request,
    call: &Call,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
) -> Result<Encoded, EncodeError> {
    fragments(request, call, texts, blobs)?.to_bytes(blobs)
}

/// 编码成片段清单（施工 V-38）：同 [`encode`]，图片、PDF 的 base64 只记 blob 和大小，发的时候才读出来编。
///
/// # Errors
///
/// 要用的 blob 执行器没交进来：先照 [`blobs_needed`] 取。
///
/// # Panics
///
/// 实际不会 panic：线上的消息里没有写不成 JSON 的东西。
pub fn fragments(
    request: &Request,
    call: &Call,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
) -> Result<Encoded<Fragments>, EncodeError> {
    // 接在最末尾的那一句（施工 RT-1，`tail.rs`）。
    let (request, _) = crate::tail::tailed(request, false);
    let request = &*request;
    let items = input::write(request, call, texts, blobs)?;
    let mut body = head(call);
    if !request.system.is_empty() {
        body.extend_from_slice(b",\"instructions\":");
        body.json(&request.system);
    }
    body.extend_from_slice(b",\"input\":[");
    let mut ranges = Vec::with_capacity(items.len());
    // 一项写完就放（施工 V-38）：线上的项是请求里的字拷出来的一份，不和请求体一起留到最后。
    for (index, item) in items.into_iter().enumerate() {
        if index > 0 {
            body.push(b',');
        }
        let start = body.len();
        body.json(&item);
        ranges.push(start..body.len());
    }
    body.push(b']');
    if let Some(tools) = wire::tools(request) {
        body.extend_from_slice(b",\"tools\":");
        body.json(&tools);
    }
    rest(&mut body, call);
    Ok(Encoded {
        body: body.finish(),
        messages: ranges,
        path: PATH.to_string(),
    })
}

/// 编码成片段清单，前面没变的历史接着用 `memo` 里留着的（施工 V-42，[`crate::Memo`]）：发出去的字节和 [`fragments`] 的一个
/// 都不差。
///
/// # Errors
///
/// 同 [`fragments`]。出错时 `memo` 整份扔掉。
pub fn fragments_memo(
    request: &Request,
    call: &Call,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
    memo: &mut Memo,
) -> Result<Encoded<Fragments>, EncodeError> {
    let (request, _) = crate::tail::tailed(request, false);
    let encoded = kept::fragments(&request, call, texts, blobs, memo);
    if encoded.is_err() {
        memo.clear();
    }
    encoded
}

/// 请求体的开头：`{"model":<模型名>`。
fn head(call: &Call) -> Builder {
    let mut body = Builder::default();
    body.extend_from_slice(b"{\"model\":");
    body.json(call.model.as_str());
    body
}

/// 工具面后面的：`store`、`stream`、输出上限、温度、思考强度，收尾的 `}`。
fn rest(body: &mut Builder, call: &Call) {
    body.extend_from_slice(b",\"store\":false,\"stream\":true");
    if let Some(limit) = call.max_output {
        body.extend_from_slice(format!(",\"max_output_tokens\":{limit}").as_bytes());
    }
    crate::write_temperature(body, call.temperature);
    effort::write(body, call.effort.as_deref());
    body.push(b'}');
}

/// 这份请求编码时要用哪些 blob：同 openai-chat（`media.rs`）。
pub fn blobs_needed(request: &Request, call: &Call) -> BTreeSet<ContentHash> {
    media::blobs_needed(request, call)
}
