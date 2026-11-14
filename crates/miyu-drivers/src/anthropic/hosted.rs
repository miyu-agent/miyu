//! 服务端工具（施工 HT-5，`docs/blueprint/drivers/anthropic.md`「怎么走：解码」第 6 条、「怎么走：编码」第 5 条）：这一家
//! 自己跑的 `web_search`、`web_fetch` 这类，线上是一块 `server_tool_use` 加一块 `*_tool_result`，并成内核的一块供应商侧工具。
//!
//! - 解码：`server_tool_use` 开块时记下它的每一格（原文）和编号，参数照工具调用攒；结果块来了照 `tool_use_id` 找到它，算出
//!   给人、给别家看的字（[`outcome`]），拼私有数据 `{"use":…,"result":…}`（[`Pending::private`]）。
//! - 编码：私有数据是这一家的、是这个形状的，照原样写回两块（[`written_back`]）。
//!
//! 结果块必须一个字节不改地写回（里面加密的内容改了、缺了这一家报 400），所以 `result` 存的是流里 `content_block` 那一格的
//! 原文。`use` 是起始块的每一格照原来的先后重写，`input` 换成收全的参数原文：起始块里的 `input` 是空对象，参数要从
//! `input_json_delta` 拼，没法逐字节照抄；别的格的值都是原文。

use std::collections::BTreeMap;
use std::fmt;

use miyu_kernel::block::{HostedTool, Private};
use miyu_kernel::id::DriverFamily;
use miyu_kernel::raw::RawJson;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use serde_json::value::RawValue;

use super::FAMILY;

/// 一块服务端工具的私有数据最多多少字节（施工 HT-5）：1 MiB。超过的不交私有数据，结果照交：日志里不存几兆的网页、PDF，
/// 这一块以后照供应商侧工具的那段字写回（施工 HT-2）。
pub const PRIVATE_BYTES: usize = 1024 * 1024;

/// 一块 JSON 对象的每一格，照原来的先后，值是原文。不经过 `serde_json::Map`：依赖里哪个 crate 打开了 `preserve_order`，
/// 键的先后也不会变（同 `wire.rs`）。
#[derive(Debug, Clone, Default)]
pub(super) struct Fields(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Fields {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Each;
        impl<'de> Visitor<'de> for Each {
            type Value = Fields;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("一个 JSON 对象")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Fields, A::Error> {
                let mut fields = Vec::new();
                while let Some(entry) = map.next_entry::<String, Box<RawValue>>()? {
                    fields.push(entry);
                }
                Ok(Fields(fields))
            }
        }
        d.deserialize_map(Each)
    }
}

/// 写回的 `use`：起始块的每一格照原来的先后，`input` 换成参数原文；起始块没有 `input` 的接在最后。
struct Use<'a> {
    fields: &'a Fields,
    input: &'a RawValue,
}

impl Serialize for Use<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        let mut wrote_input = false;
        for (key, value) in &self.fields.0 {
            if key == "input" {
                map.serialize_entry(key, self.input)?;
                wrote_input = true;
            } else {
                map.serialize_entry(key, value)?;
            }
        }
        if !wrote_input {
            map.serialize_entry("input", self.input)?;
        }
        map.end()
    }
}

/// 开着、等结果的一块服务端工具。
#[derive(Debug, Clone)]
pub(super) struct Pending {
    /// 内核的块编号。
    pub(super) index: usize,
    /// 线上的编号，结果块的 `tool_use_id` 照它找。
    pub(super) id: String,
    /// 起始块的每一格；起始块读不成对象的（不会有）没有，也就不交私有数据。
    pub(super) fields: Option<Fields>,
    /// 攒起来的参数原文。
    pub(super) args: String,
}

/// 一条 `content_block_start` 里 `content_block` 那一格的原文。
#[derive(Deserialize)]
struct Start<'a> {
    #[serde(borrow)]
    content_block: &'a RawValue,
}

/// `content_block_start` 这一条事件（`data` 原文）里 `content_block` 的原文；读不成的没有。
pub(super) fn block_text(data: &str) -> Option<&str> {
    serde_json::from_str::<Start<'_>>(data)
        .ok()
        .map(|start| start.content_block.get())
}

impl Pending {
    /// 照起始块开一块：`block` 是起始块的原文。
    pub(super) fn new(index: usize, id: &str, block: Option<&str>) -> Pending {
        Pending {
            index,
            id: id.to_string(),
            fields: block.and_then(|block| serde_json::from_str(block).ok()),
            args: String::new(),
        }
    }

    /// 私有数据 `{"use":…,"result":…}`：`result` 是结果块的原文。参数不是合法 JSON 的、整份超过 [`PRIVATE_BYTES`] 的
    /// 没有。没收到参数的，`input` 照起始块里的（起始块也没有的写 `{}`）。
    pub(super) fn private(&self, result: &str) -> Option<Private> {
        #[derive(Serialize)]
        struct Data<'a> {
            #[serde(rename = "use")]
            used: Use<'a>,
            result: &'a RawValue,
        }
        let fields = self.fields.as_ref()?;
        let given = fields.0.iter().find(|(key, _)| key == "input");
        let input: &RawValue = match (self.args.trim(), given) {
            ("", Some((_, input))) => input,
            ("", None) => empty_object(),
            (args, _) => serde_json::from_str(args).ok()?,
        };
        let result: &RawValue = serde_json::from_str(result).ok()?;
        let data = serde_json::to_string(&Data {
            used: Use { fields, input },
            result,
        })
        .ok()?;
        if data.len() > PRIVATE_BYTES {
            return None;
        }
        Some(Private {
            driver: DriverFamily::parse(FAMILY).expect("驱动家族合写法"),
            data: serde_json::from_str::<RawJson>(&data).ok()?,
        })
    }
}

/// `value` 里字符串的那一格。
fn field<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(Value::as_str)
}

/// `{}`。
fn empty_object() -> &'static RawValue {
    serde_json::from_str("{}").expect("{} 是 JSON")
}

/// 结果块的种类：以 `_tool_result` 结尾的才是。
pub(super) fn is_result(kind: &str) -> bool {
    kind.ends_with("_tool_result")
}

/// 结果的字和失败了没有（「怎么走：解码」第 6 条第 3 点）：`kind` 是结果块的种类，`block` 是它的原文。
pub(super) fn outcome(kind: &str, block: &str) -> (String, bool) {
    #[derive(Deserialize)]
    struct ResultBlock<'a> {
        #[serde(borrow, default)]
        content: Option<&'a RawValue>,
    }
    let raw = serde_json::from_str::<ResultBlock<'_>>(block)
        .ok()
        .and_then(|result| result.content)
        .map_or("null", RawValue::get);
    let content: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    let failed = field(&content, "type").is_some_and(|kind| kind.ends_with("_error"));
    let said = match kind {
        "web_search_tool_result" | "web_fetch_tool_result" if failed => {
            field(&content, "error_code").map(str::to_string)
        }
        "web_search_tool_result" => content.as_array().map(|hits| {
            hits.iter()
                .map(|hit| {
                    let title = field(hit, "title").unwrap_or_default();
                    let url = field(hit, "url").unwrap_or_default();
                    format!("{title} - {url}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }),
        "web_fetch_tool_result" => fetched(&content),
        _ => None,
    };
    (said.unwrap_or_else(|| raw.to_string()), failed)
}

/// 抓到的网页写成的字：`<url>`、有标题的下一行 `<title>`、空一行、正文；正文不是文本的写 `(<media_type>)`。不是
/// `web_fetch_result` 的没有。
fn fetched(content: &Value) -> Option<String> {
    if field(content, "type") != Some("web_fetch_result") {
        return None;
    }
    let document = &content["content"];
    let source = &document["source"];
    let mut said = field(content, "url").unwrap_or_default().to_string();
    if let Some(title) = field(document, "title") {
        said.push('\n');
        said.push_str(title);
    }
    said.push_str("\n\n");
    match field(source, "type") {
        Some("text") => said.push_str(field(source, "data").unwrap_or_default()),
        _ => {
            said.push('(');
            said.push_str(field(source, "media_type").unwrap_or_default());
            said.push(')');
        }
    }
    Some(said)
}

/// 写回的两块（「怎么走：编码」第 5 条）：私有数据是这一家的、是 `{"use":{…},"result":{…}}` 的，`use`、`result` 的原文；
/// 别的没有，照 HT-2 写成字。
pub(super) fn written_back(hosted: &HostedTool) -> Option<[Box<RawValue>; 2]> {
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "use")]
        used: Box<RawValue>,
        result: Box<RawValue>,
    }
    let private = hosted
        .private
        .as_ref()
        .filter(|private| private.driver.as_str() == FAMILY)?;
    let data: Data = serde_json::from_str(private.data.get()).ok()?;
    let object =
        |raw: &RawValue| serde_json::from_str::<BTreeMap<String, IgnoredAny>>(raw.get()).is_ok();
    (object(&data.used) && object(&data.result)).then_some([data.used, data.result])
}
