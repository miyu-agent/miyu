//! 查扩展驱动的编码稳不稳（施工 MD-3，设计 66 第七节第 2 条，core1 11-07 照推荐定）：`driver.check` 照这里的样本调扩展的
//! `driver.encode`，照这里的比法比字节。样本在核心里用代码造（发行版不带 `docs/`，统一的请求也读不回来），参数和真发的
//! `driver.call` 一个写法（`extension/wire.rs` 的 `params`）。
//!
//! - 每一份（[`samples`]）编两次、重启扩展再编一次，三次要逐字节相同（[`first_difference`]）；
//! - 只在末尾加一条消息的一对（[`pair`]）：最后一条稳定区消息里放着记号字 [`STABLE_MARK`]（纯 ASCII 的字母数字，JSON 里怎么写都
//!   不会变），两份编出来的字节都要找得到它，到它为止逐字节相同（[`stable_prefix`]）。找不到的照「没法查稳定区」报红，不当过。

use std::collections::BTreeMap;

use miyu_drivers::{Call, Inputs};
use miyu_kernel::block::{Block, Image, Private, Reasoning, Text, ToolCall};
use miyu_kernel::id::{CallId, ContentHash, DriverFamily, MediaType, ModelName};
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request, ToolSpec};

use super::wire::{self, Outgoing};

/// 稳定区结束的记号字：放在样本最后一条稳定区消息的末尾。
pub const STABLE_MARK: &str = "miyustableend7f3a";

/// 一份样本：名字（报的时候说是哪一份）、`driver.encode` 的参数原文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    /// 名字：`plain`、`tools` 这样。
    pub name: &'static str,
    /// 参数原文，一个 JSON 对象。
    pub params: String,
}

/// 驱动 `driver`（`<包编号>/<驱动名>`）的几份样本，照金样的几种：纯文字、工具面、带私有数据的思考和工具调用、看不了图的转述、
/// 续写。
///
/// # Panics
///
/// 不会：样本里的名字、类型、JSON 都是写死的、合写法的。
pub fn samples(driver: &str) -> Vec<Sample> {
    let family = DriverFamily::parse(driver).ok();
    let mut tools = base();
    tools.tools = vec![ToolSpec {
        name: "read".to_string(),
        description: "Read a file.".to_string(),
        parameters: RawJson::of(
            &serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        )
        .expect("是 JSON"),
    }];
    let mut private = tools.clone();
    let call_id = CallId::parse("call_3_1").expect("合写法");
    let owned = |data: &str| {
        family.clone().map(|driver| Private {
            driver,
            data: RawJson::of(&serde_json::json!(data)).expect("是 JSON"),
        })
    };
    private.messages.push(Message::Assistant {
        blocks: vec![
            Block::Reasoning(Reasoning {
                text: "Look at the file first.".to_string(),
                private: owned("sig-1"),
            }),
            Block::ToolCall(ToolCall {
                call_id,
                name: "read".to_string(),
                args: r#"{"path":"a.txt"}"#.to_string(),
                private: owned("call-1"),
            }),
        ],
    });
    private.messages.push(Message::Tool {
        call_id,
        error: false,
        blocks: vec![text("hello")],
    });
    let mut image = base();
    let blob = ContentHash::of(b"miyu driver check image");
    image.messages.push(Message::User {
        blocks: vec![
            Block::Image(Image {
                blob: blob.clone(),
                name: None,
                media_type: MediaType::parse("image/png").expect("合写法"),
                width: 2,
                height: 2,
                path: None,
            }),
            text("What is in it?"),
        ],
    });
    image.described = BTreeMap::from([(blob, "A red square.".to_string())]);
    let mut continuation = base();
    continuation.messages.push(Message::Assistant {
        blocks: vec![text("The answer is")],
    });
    continuation.messages.push(Message::User {
        blocks: vec![text("go on")],
    });
    continuation.continuation = true;
    [
        ("plain", base()),
        ("tools", tools),
        ("private", private),
        ("image", image),
        ("continuation", continuation),
    ]
    .into_iter()
    .map(|(name, request)| Sample {
        name,
        params: params(driver, &request),
    })
    .collect()
}

/// 只在末尾加一条消息的两份：短的那一份、多一条人话的那一份。
pub fn pair(driver: &str) -> [Sample; 2] {
    let short = base();
    let mut long = short.clone();
    long.messages.push(Message::User {
        blocks: vec![text("And one more thing.")],
    });
    [
        Sample {
            name: "pair-short",
            params: params(driver, &short),
        },
        Sample {
            name: "pair-long",
            params: params(driver, &long),
        },
    ]
}

/// 两份字节头一处不同在第几个字节（从 0 数）；一样的没有。一份是另一份的开头的，不同在短的那一份的长度。
pub fn first_difference(one: &[u8], other: &[u8]) -> Option<usize> {
    let at = one.iter().zip(other).position(|(a, b)| a != b);
    match (at, one.len() == other.len()) {
        (Some(at), _) => Some(at),
        (None, true) => None,
        (None, false) => Some(one.len().min(other.len())),
    }
}

/// 只在末尾加一条消息的两份编出来的字节，到记号字为止要相同。
///
/// # Errors
///
/// 哪一份找不到记号字（没法查稳定区）、到记号字为止有不同的：英文的一句，写明是哪一份、第几个字节。
pub fn stable_prefix(short: &[u8], long: &[u8]) -> Result<(), String> {
    let end = |body: &[u8], which: &str| {
        body.windows(STABLE_MARK.len())
            .position(|window| window == STABLE_MARK.as_bytes())
            .map(|at| at + STABLE_MARK.len())
            .ok_or_else(|| {
                format!("the stable region can't be checked: the {which} body has no {STABLE_MARK}")
            })
    };
    let (short_end, long_end) = (end(short, "shorter")?, end(long, "longer")?);
    match first_difference(&short[..short_end], &long[..long_end]) {
        None => Ok(()),
        Some(at) => Err(format!(
            "adding a message at the end changed byte {at}, before the end of the stable region"
        )),
    }
}

/// 样本的底子：一句 system、示范的一来一回（稳定区，回的那一句末尾是记号字）、人说的一句。
fn base() -> Request {
    Request {
        tools: Vec::new(),
        system: "You are a helpful assistant.".to_string(),
        messages: vec![
            Message::User {
                blocks: vec![text("Say hi.")],
            },
            Message::Assistant {
                blocks: vec![text(&format!("Hi. {STABLE_MARK}"))],
            },
            Message::User {
                blocks: vec![text("How is the weather today?")],
            },
        ],
        stable: 2,
        continuation: false,
        described: BTreeMap::new(),
        output_cap: None,
        used: None,
        tail: None,
    }
}

fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

/// 照真发的写法写参数：编号固定（扩展不该把它编进去），模型、输出上限固定，不交 key、地址、图。
fn params(driver: &str, request: &Request) -> String {
    let bytes = request.canonical_bytes();
    let call = Call {
        model: ModelName::parse("check-model").expect("合写法"),
        max_output: Some(256),
        inputs: Inputs::default(),
        effort: None,
        temperature: None,
    };
    wire::params(&Outgoing {
        call: "check",
        driver,
        request: &bytes,
        fingerprint: &request.fingerprint(),
        params: &call,
        key: None,
        base_url: None,
        blobs: &BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests;
