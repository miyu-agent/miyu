//! 供应商那边跑的搜索（施工 DSR-4，`internal/blueprint/drivers/openai-responses.md`「怎么走：解码」第 5 条）：输出里一项
//! `web_search_call` 是 DeepSeek（或者 OpenAI 自己）替模型跑的一次搜索、读网页，并成内核的一块供应商侧工具（设计 71）。
//!
//! - 解码：项开始时（`response.output_item.added`）开块，参数是这一项 `action` 的原文；项完了（`output_item.done`）照那里
//!   的整项算出结果的字（[`outcome`]），当场收块。
//! - 编码：这里什么都不做。这一块不带私有数据，下一次请求照 HT-2 写成那段字、接进上一轮的正文（`input.rs`）。2026-11-19
//!   实测：DeepSeek 收到原样带回的 `web_search_call` 不报错，可是不把它放进上下文（带回 6 项，输入只有 1,366 token），
//!   下一轮模型说自己没有搜索过；写成字的（输入 3,487 token），它说得出上一轮搜了哪几句、开了哪几页。

use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;

/// 输出项的种类：是它的才是供应商那边跑的搜索。
pub(super) const ITEM: &str = "web_search_call";

/// 一条 `output_item.added`、`output_item.done` 事件里要原文的几格。
#[derive(Deserialize)]
struct Event<'a> {
    #[serde(borrow)]
    item: &'a RawValue,
}

/// 一项里要原文的那一格。
#[derive(Deserialize)]
struct Item<'a> {
    #[serde(borrow, default)]
    action: Option<&'a RawValue>,
}

/// 事件（`data` 原文）里 `item` 那一格的原文；读不成的没有。
pub(super) fn item_text(data: &str) -> Option<&str> {
    serde_json::from_str::<Event<'_>>(data)
        .ok()
        .map(|event| event.item.get())
}

/// 内核这一块的工具名：DeepSeek 的项带 `kind`，打开网页、在网页里找字的（`action.type` 是 `open_page`、`find_in_page`，
/// 深度搜索里的和读链接的都是）是 `deepseek_read_url`，别的是 `deepseek_web_search`；OpenAI 自己的项没有 `kind`，是
/// `web_search`。视图照名字认它像哪件核心工具（`resources/core/view.json` 的 `hosted`）。
pub(super) fn name(item: &Value) -> &'static str {
    if item.get("kind").is_none() {
        return "web_search";
    }
    match field(&item["action"], "type") {
        Some("open_page" | "find_in_page") => "deepseek_read_url",
        _ => "deepseek_web_search",
    }
}

/// 参数：这一项 `action` 的原文，一个字节不改；没有 `action` 的是空的。开块的那一条事件里的就是：项开始时 `action` 已经在，
/// 项完了时多出来的 `sources` 是结果，归 [`outcome`]。
pub(super) fn args(item: &str) -> &str {
    serde_json::from_str::<Item<'_>>(item)
        .ok()
        .and_then(|item| item.action)
        .map_or("", RawValue::get)
}

/// 结果的字和失败了没有：`item` 是项完了时的整项，`raw` 是它的原文。
///
/// - `status` 是 `failed` 的：失败，字是 `error.code`。
/// - 搜索（`action.type` 是 `search`）：`action.sources` 一条一行，`<标题> - <网址>`，同 Anthropic 搜索结果的写法。
/// - 读网页（`open_page`）：`<网址>`，有标题的下一行 `<标题>`，同 Anthropic 读网页的头两行；网页内容这一家不交。在网页里找
///   （`find_in_page`）再加一行 `pattern: <找的字>`。网址先取 `page.url`，没有的取 `action.url`。
/// - 别的、缺了要的格的：整项原文，同 Anthropic 认不出的结果。
pub(super) fn outcome(item: &Value, raw: &str) -> (String, bool) {
    let failed = field(item, "status") == Some("failed");
    let action = &item["action"];
    let said = match field(action, "type") {
        _ if failed => item
            .pointer("/error/code")
            .and_then(Value::as_str)
            .map(str::to_string),
        Some("search") => action["sources"].as_array().map(|sources| {
            sources
                .iter()
                .map(|source| {
                    let title = field(source, "title").unwrap_or_default();
                    let url = field(source, "url").unwrap_or_default();
                    format!("{title} - {url}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }),
        Some("open_page") => Some(page(item, action)),
        Some("find_in_page") => {
            let pattern = field(action, "pattern").unwrap_or_default();
            Some(format!("{}\npattern: {pattern}", page(item, action)))
        }
        _ => None,
    };
    (said.unwrap_or_else(|| raw.to_string()), failed)
}

/// 读到的网页：`<网址>`，有标题的下一行 `<标题>`。
fn page(item: &Value, action: &Value) -> String {
    let page = &item["page"];
    let mut said = field(page, "url")
        .or_else(|| field(action, "url"))
        .unwrap_or_default()
        .to_string();
    if let Some(title) = field(page, "title") {
        said.push('\n');
        said.push_str(title);
    }
    said
}

/// `value` 里字符串的那一格。
fn field<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(Value::as_str)
}
