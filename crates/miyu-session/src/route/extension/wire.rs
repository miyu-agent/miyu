//! 扩展驱动线上的写法（施工 MD-2，设计 66 第五节）：`driver.call` 的参数怎么写，`driver.delta` 的增量、`driver.call` 的回答怎么读。
//!
//! - 参数：`{"call", "driver", "request", "fingerprint", "params", "key"?, "base_url"?, "blobs"?}`。`request` 是规范字节原样，拼进
//!   去、不重新解析；指纹的哈希写成 16 位十六进制的字（JSON 的数到 2⁵³ 以上就不准了）；`blobs` 只有声明了 `images` 的才有。
//! - 请求：别家驱动的私有数据拿掉（设计 66 第四节第 3 条）；供应商侧工具带着这个家族的私有数据的原样交，别家的、没带私有
//!   数据的换成一块正文，同内置驱动写的那一段字（施工 HT-2，设计 71 第三节第 5 条）。没有要改的原样交、不拷贝。
//! - 增量：一条一样，和内核的 [`Delta`] 一一对应；私有数据记成这个驱动的家族（`<包编号>/<驱动名>`）。
//! - 回答：`{"usage"?, "error"?, "sent"?}`；分类照设计第八节第 3 条的七种认，不认识的当 `unclassified`，原话截到 500 个字，
//!   要等多久只认限速的。

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use miyu_drivers::classify::Classified;
use miyu_drivers::{Call, DriverTexts};
use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::block::{Block, HostedTool, Private, Text};
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::id::{ContentHash, DriverFamily};
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Fingerprint, Message, Request};

/// 第三方写的原话最多留多少个字（设计 66 第八节第 3 条）：只给人看，不进模型的上下文。
const MESSAGE_CHARS: usize = 500;

/// 一张交过去的图（声明了 `images` 的驱动）。
#[derive(Serialize)]
pub(super) struct Picture {
    pub(super) media_type: String,
    /// base64。
    pub(super) data: String,
}

/// 参数里除了 `request` 的几格；`request` 拼在最前面。
#[derive(Serialize)]
struct Rest<'a> {
    call: &'a str,
    driver: &'a str,
    fingerprint: Hashes,
    params: Params<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_url: Option<&'a str>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    blobs: &'a BTreeMap<String, Picture>,
}

/// 指纹：工具面、system、每条消息各一个哈希。
#[derive(Serialize)]
struct Hashes {
    tools: String,
    system: String,
    messages: Vec<String>,
}

/// `Call` 里交过去的四样。
#[derive(Serialize)]
struct Params<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

/// 这一次要交过去的：编号、驱动、请求的规范字节和它的指纹、调用的几样、这一家的 key 和地址、图。
pub(super) struct Outgoing<'a> {
    pub(super) call: &'a str,
    pub(super) driver: &'a str,
    pub(super) request: &'a [u8],
    pub(super) fingerprint: &'a Fingerprint,
    pub(super) params: &'a Call,
    pub(super) key: Option<&'a str>,
    pub(super) base_url: Option<&'a str>,
    pub(super) blobs: &'a BTreeMap<String, Picture>,
}

/// 写成参数的原文：一个 JSON 对象，`request` 是规范字节原样（规范字节一定是 JSON 对象，不再解析一遍）。
pub(super) fn params(outgoing: &Outgoing<'_>) -> String {
    let (tools, system, messages) = outgoing.fingerprint.hashes();
    let hex = |hash: u64| format!("{hash:016x}");
    let rest = Rest {
        call: outgoing.call,
        driver: outgoing.driver,
        fingerprint: Hashes {
            tools: hex(tools),
            system: hex(system),
            messages: messages.map(hex).collect(),
        },
        params: Params {
            model: outgoing.params.model.as_str(),
            max_output: outgoing.params.max_output,
            effort: outgoing.params.effort.as_deref(),
            temperature: outgoing.params.temperature,
        },
        key: outgoing.key,
        base_url: outgoing.base_url,
        blobs: outgoing.blobs,
    };
    let rest = serde_json::to_string(&rest).expect("参数里没有写不成 JSON 的东西");
    let request = String::from_utf8_lossy(outgoing.request);
    // `rest` 是一个不空的对象（至少有 `call`）：`{` 后面接上 `"request":<规范字节>,`。
    let mut text = String::with_capacity(rest.len() + request.len() + 12);
    text.push_str("{\"request\":");
    text.push_str(&request);
    text.push(',');
    text.push_str(&rest[1..]);
    text
}

/// 交给家族是 `family` 的扩展驱动的请求（见模块的说明）：没有要改的原样交回、不拷贝；有的拷一份，先把不是它自己家的供应商
/// 侧工具换成字（[`retell`]），再拿掉别家的私有数据（[`strip`]）。
pub(super) fn handed<'a>(
    request: Cow<'a, Request>,
    family: &DriverFamily,
    texts: &DriverTexts,
) -> Cow<'a, Request> {
    if !foreign(&request, family) {
        return request;
    }
    let mut copy = request.into_owned();
    retell(&mut copy, family, texts);
    strip(&mut copy, family);
    Cow::Owned(copy)
}

/// 请求交出去以前要不要改：有别家驱动的私有数据（设计 66 第四节第 3 条：只回给同一个家族），或者有不是这个家族原样收回
/// 的供应商侧工具（施工 HT-2：没带私有数据的也要换成字）。
pub(super) fn foreign(request: &Request, family: &DriverFamily) -> bool {
    request.messages.iter().flat_map(blocks).any(|block| {
        private(block).is_some_and(|private| private.driver != *family)
            || matches!(block, Block::HostedTool(hosted) if !own(hosted, family))
    })
}

/// 供应商侧工具是不是这个家族原样收回的：认「自己家」只看块上的私有数据（施工 HT-2）。
fn own(hosted: &HostedTool, family: &DriverFamily) -> bool {
    hosted
        .private
        .as_ref()
        .is_some_and(|private| private.driver == *family)
}

/// 不是这个家族的供应商侧工具换成一块正文，内容是内置驱动写的那一段字（[`DriverTexts::hosted_tool`]），位置不变。占位里没有
/// 那几份的（以前造的快照）拿掉它：别家的私有数据不能原样交给扩展，又没有登记过的字可写（施工 HT-2）。
pub(super) fn retell(request: &mut Request, family: &DriverFamily, texts: &DriverTexts) {
    for message in &mut request.messages {
        let blocks = match message {
            Message::User { blocks }
            | Message::Assistant { blocks }
            | Message::Tool { blocks, .. } => blocks,
        };
        let told = std::mem::take(blocks)
            .into_iter()
            .filter_map(|block| match block {
                Block::HostedTool(hosted) if !own(&hosted, family) => texts
                    .hosted_tool(&hosted)
                    .map(|text| Block::Text(Text { text })),
                block => Some(block),
            });
        *blocks = told.collect();
    }
}

/// 拿掉别家驱动的私有数据：块还在，只是不带它。
pub(super) fn strip(request: &mut Request, family: &DriverFamily) {
    for message in &mut request.messages {
        let blocks = match message {
            Message::User { blocks }
            | Message::Assistant { blocks }
            | Message::Tool { blocks, .. } => blocks,
        };
        for block in blocks {
            let slot = match block {
                Block::Reasoning(reasoning) => &mut reasoning.private,
                Block::ToolCall(call) => &mut call.private,
                Block::HostedTool(hosted) => &mut hosted.private,
                _ => continue,
            };
            if slot
                .as_ref()
                .is_some_and(|private| private.driver != *family)
            {
                *slot = None;
            }
        }
    }
}

/// 请求里的图：blob 和类型，照先后、去重。
pub(super) fn images(request: &Request) -> BTreeMap<ContentHash, String> {
    request
        .messages
        .iter()
        .flat_map(blocks)
        .filter_map(|block| match block {
            Block::Image(image) => Some((image.blob.clone(), image.media_type.to_string())),
            _ => None,
        })
        .collect()
}

fn blocks(message: &Message) -> &[Block] {
    match message {
        Message::User { blocks } | Message::Assistant { blocks } | Message::Tool { blocks, .. } => {
            blocks
        }
    }
}

fn private(block: &Block) -> Option<&Private> {
    match block {
        Block::Reasoning(reasoning) => reasoning.private.as_ref(),
        Block::ToolCall(call) => call.private.as_ref(),
        Block::HostedTool(hosted) => hosted.private.as_ref(),
        _ => None,
    }
}

/// 线上的一条增量。
#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Wire {
    Start { index: usize, kind: WireKind },
    Text { index: usize, text: String },
    Private { index: usize, data: Value },
    End { index: usize },
    Truncated,
}

/// 一块的种类：`text`、`reasoning`、`{"tool_call": {"name"}}`。
#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum WireKind {
    Text,
    Reasoning,
    ToolCall { name: String },
}

/// 读一份 `driver.delta` 的 `deltas`：私有数据记成 `family` 的。
///
/// # Errors
///
/// 不是数组、哪一条写法不对：英文的一句。
pub(super) fn deltas(batch: Value, family: &DriverFamily) -> Result<Vec<Delta>, String> {
    let wires: Vec<Wire> = serde_json::from_value(batch).map_err(|error| error.to_string())?;
    wires
        .into_iter()
        .map(|wire| {
            Ok(match wire {
                Wire::Start { index, kind } => Delta::Start {
                    index,
                    kind: match kind {
                        WireKind::Text => Kind::Text,
                        WireKind::Reasoning => Kind::Reasoning,
                        WireKind::ToolCall { name } => Kind::ToolCall { name },
                    },
                },
                Wire::Text { index, text } => Delta::Text { index, text },
                Wire::Private { index, data } => Delta::Private {
                    index,
                    private: Private {
                        driver: family.clone(),
                        data: RawJson::of(&data).ok_or("private data is not JSON")?,
                    },
                },
                Wire::End { index } => Delta::End { index },
                Wire::Truncated => Delta::Truncated,
            })
        })
        .collect()
}

/// `driver.call` 的回答。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    #[serde(default)]
    usage: Option<Usage>,
    #[serde(default)]
    error: Option<WireError>,
    #[serde(default)]
    sent: Option<ContentHash>,
}

/// 回答里的出错。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireError {
    class: String,
    message: String,
    #[serde(default)]
    retry_after_ms: Option<u64>,
}

/// 读出来的回答：用量、出错、真发出去的字节的哈希。
pub(super) struct Said {
    pub(super) usage: Option<Usage>,
    pub(super) error: Option<Classified>,
    pub(super) sent: Option<ContentHash>,
}

/// 读回答。
///
/// # Errors
///
/// 写法不对：英文的一句。
pub(super) fn answer(result: Value) -> Result<Said, String> {
    // serde 把空的数组也读成「什么都没报」的结构：回答一定是一个对象。
    if !result.is_object() {
        return Err("the answer is not an object".to_string());
    }
    let answer: Answer = serde_json::from_value(result).map_err(|error| error.to_string())?;
    Ok(Said {
        usage: answer.usage,
        error: answer.error.map(|error| {
            let class = class(&error.class);
            let retry_after_ms = error
                .retry_after_ms
                .filter(|_| class == ErrorClass::RateLimited);
            classified(class, &error.message, retry_after_ms)
        }),
        sent: answer.sent,
    })
}

/// 分类：八种照名字认，不认识的当 `unclassified`（内核里的字面是 `other`）。
fn class(text: &str) -> ErrorClass {
    match text {
        "retryable" => ErrorClass::Retryable,
        "stalled" => ErrorClass::Stalled,
        "rate_limited" => ErrorClass::RateLimited,
        "context_too_long" => ErrorClass::ContextTooLong,
        "auth" => ErrorClass::Auth,
        "quota" => ErrorClass::Quota,
        "content_policy" => ErrorClass::ContentPolicy,
        _ => ErrorClass::Unclassified,
    }
}

/// 一次出错：分类、原话（截到 500 个字）、要等多久。
pub(super) fn classified(
    class: ErrorClass,
    message: &str,
    retry_after_ms: Option<u64>,
) -> Classified {
    Classified {
        error: CallError {
            class,
            message: message.chars().take(MESSAGE_CHARS).collect(),
            status: None,
            retry_after_ms,
        },
        excess: None,
        limit: None,
    }
}

#[cfg(test)]
pub(super) mod tests;
