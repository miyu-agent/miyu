//! 供应商那边跑的搜索（施工 DSR-4，`internal/blueprint/drivers/openai-responses.md`「怎么走：解码」第 5 条、「怎么走：编码」
//! 第 4 条）：DeepSeek 官方 Responses 接口真的流（样本 `streams/deepseek-web-search.sse`，2026-11-19 录的，只有公开的
//! 搜索结果）解出来，每一项 `web_search_call` 是一块供应商侧工具：名字、参数原文、结果的字、整项原文的私有数据；思考、正文
//! 照旧；用量多搜索、抓取的次数。编码回去每一项照原样、照原来的先后写回，留着历史的编码和整份编码一字不差；别家的、超过
//! 上限的照 HT-2 写成字。

use std::collections::BTreeMap;
use std::fs;

use crate::support::{family_dir, gpt, text, texts};
use miyu_drivers::openai_responses::{Decoder, encode, fragments_memo};
use miyu_drivers::{Call, Inputs};
use miyu_drivers::{Ending, Memo};
use miyu_kernel::accumulate::{Accumulator, Delta, Kind};
use miyu_kernel::block::{Block, HostedTool, Outcome};
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Message, Request};
use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;

fn sample() -> Vec<u8> {
    let path = family_dir("openai-responses").join("streams/deepseek-web-search.sse");
    fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()))
}

/// 一片一片喂进去，再收尾；交回全部增量和收尾。
fn decode(chunks: &[&[u8]]) -> (Vec<Delta>, Ending) {
    let mut decoder = Decoder::new();
    let mut deltas = Vec::new();
    for chunk in chunks {
        deltas.extend(decoder.feed(chunk));
    }
    let mut ending = decoder.finish();
    assert!(ending.error.is_none(), "{:?}", ending.error);
    deltas.append(&mut ending.deltas);
    (deltas, ending)
}

fn reply(stream: &[u8]) -> Vec<Block> {
    let mut accumulator = Accumulator::default();
    for delta in decode(&[stream]).0 {
        accumulator.apply(delta).expect("累积器收得下");
    }
    accumulator.finish(Seq::FIRST)
}

/// 流里种类是 `event` 的、项是 `web_search_call` 的每一条事件里 `item` 那一格的原文，照先后：照字切，不经过 `Value`。
fn wire_items(stream: &[u8], event: &str) -> Vec<String> {
    #[derive(Deserialize)]
    struct Data<'a> {
        #[serde(borrow)]
        item: &'a RawValue,
    }
    String::from_utf8(stream.to_vec())
        .expect("UTF-8")
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|data| data.contains(&format!(r#""type":"{event}""#)))
        .filter(|data| data.contains(r#""item":{"type":"web_search_call""#))
        .map(|data| {
            let parsed: Data<'_> = serde_json::from_str(data).expect("是 JSON");
            parsed.item.get().to_string()
        })
        .collect()
}

/// 一项原文里 `action` 那一格的原文。
fn action(item: &str) -> String {
    #[derive(Deserialize)]
    struct Item<'a> {
        #[serde(borrow)]
        action: &'a RawValue,
    }
    let parsed: Item<'_> = serde_json::from_str(item).expect("是 JSON");
    parsed.action.get().to_string()
}

fn hosted_blocks(blocks: &[Block]) -> Vec<&HostedTool> {
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::HostedTool(hosted) => Some(hosted),
            _ => None,
        })
        .collect()
}

#[test]
fn the_real_stream_gives_one_hosted_block_per_search_in_order() {
    let stream = sample();
    let blocks = reply(&stream);
    let kinds: Vec<&str> = blocks
        .iter()
        .map(|block| match block {
            Block::Reasoning(_) => "reasoning",
            Block::HostedTool(_) => "hosted",
            Block::Text(_) => "text",
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "reasoning",
            "hosted",
            "reasoning",
            "hosted",
            "hosted",
            "hosted",
            "reasoning",
            "hosted",
            "hosted",
            "reasoning",
            "text"
        ],
        "照项第一次出现的先后：先全部 added，再逐个完成的三项也照 added 的先后"
    );
    let added = wire_items(&stream, "response.output_item.added");
    let done = wire_items(&stream, "response.output_item.done");
    let hosted = hosted_blocks(&blocks);
    assert_eq!((added.len(), done.len(), hosted.len()), (6, 6, 6));
    // `done` 照完成的先后，块照开始的先后：照项的 `id` 对上。
    for (block, opening) in hosted.iter().zip(&added) {
        assert_eq!(block.args, action(opening));
        assert!(block.private.is_none(), "不存私有数据：下一轮写成字");
    }
    // 六项的 `kind` 都是 `deep_search`：搜索的是 `deepseek_web_search`，打开网页的是 `deepseek_read_url`。
    let names: Vec<&str> = hosted.iter().map(|block| block.name.as_str()).collect();
    let (search, read) = ("deepseek_web_search", "deepseek_read_url");
    assert_eq!(names, [search, read, read, search, read, read]);
    assert_eq!(
        hosted[0].args,
        r#"{"type":"search","queries":["latest stable Rust version release date 2025","Rust blog announce Rust 1.90 stable release"]}"#,
        "参数是开块时的 action，不带结果 sources"
    );
    let first = hosted[0].outcome.as_ref().expect("有结果");
    assert!(!first.failed && !first.cut);
    let lines: Vec<&str> = first.text.lines().collect();
    assert_eq!(lines.len(), 20, "一条来源一行");
    assert_eq!(
        lines[0],
        "The Rust Release Announcements - https://blog.rust-lang.org/releases/"
    );
    assert_eq!(
        hosted[1].outcome,
        Some(Outcome {
            text: "https://blog.rust-lang.org/tags/release/\nPosts tagged \"release\" | Rust Blog"
                .to_string(),
            failed: false,
            cut: false,
        })
    );
    assert_eq!(
        hosted[5].outcome,
        Some(Outcome {
            text: "unavailable".to_string(),
            failed: true,
            cut: false,
        }),
        "失败的写错误码"
    );
    let Some(Block::Text(said)) = blocks.last() else {
        panic!("最后一块是正文");
    };
    assert_eq!(
        said.text,
        "The latest stable version of Rust is **1.99.1**, released on **October 15, 2026**.",
        "引用不理，正文照字"
    );
    let Block::Reasoning(thought) = &blocks[0] else {
        panic!("第一块是思考");
    };
    assert!(
        thought
            .text
            .starts_with("The user asks: latest stable version of Rust")
    );
    assert!(thought.private.is_none(), "DeepSeek 的思考没有加密内容");
}

#[test]
fn a_search_opens_at_added_and_closes_at_done() {
    let stream = sample();
    let (deltas, ending) = decode(&[&stream]);
    // 第一项搜索是第 1 块：项开始时开块、交参数，项完了时交结果并收块，这一块再没有别的增量。
    let of_one: Vec<&Delta> = deltas
        .iter()
        .filter(|delta| match delta {
            Delta::Start { index, .. }
            | Delta::Text { index, .. }
            | Delta::Private { index, .. }
            | Delta::Outcome { index, .. }
            | Delta::End { index } => *index == 1,
            Delta::Truncated => false,
        })
        .collect();
    assert!(matches!(
        of_one[0],
        Delta::Start { kind: Kind::HostedTool { name }, .. } if name == "deepseek_web_search"
    ));
    assert!(matches!(of_one[1], Delta::Text { .. }));
    assert!(matches!(of_one[2], Delta::Outcome { failed: false, .. }));
    assert!(matches!(of_one[3], Delta::End { .. }));
    assert_eq!(of_one.len(), 4, "不交私有数据");
    let usage = ending.usage.expect("有用量");
    assert_eq!(usage.reasoning, Some(599));
    assert_eq!((usage.cache_read, usage.uncached), (13184, 21989 - 13184));
    // 从哪里切开喂都一样（整份 19 万字节，挑几处切）。
    for cut in (0..stream.len()).step_by(997) {
        assert_eq!(
            decode(&[&stream[..cut], &stream[cut..]]).0,
            deltas,
            "切在 {cut}"
        );
    }
    let bytes: Vec<&[u8]> = stream.chunks(1).collect();
    assert_eq!(decode(&bytes).0, deltas, "一个字节一个字节地喂");
}

fn request(reply: Vec<Block>) -> Request {
    Request {
        tools: vec![],
        system: "You are a helpful assistant.".to_string(),
        messages: vec![
            Message::User {
                blocks: vec![text("Rust 最新是哪一版？")],
            },
            Message::Assistant { blocks: reply },
            Message::User {
                blocks: vec![text("谢谢")],
            },
        ],
        stable: 0,
        continuation: false,
        described: Default::default(),
        output_cap: None,
        used: None,
        tail: None,
    }
}

/// 声明了 DeepSeek 的搜索的一次调用（设计 72 激活时的样子）。
fn searching() -> Call {
    Call {
        hosted: vec!["deepseek_web_search".to_string()],
        ..gpt(Inputs::default(), None)
    }
}

fn whole(request: &Request) -> Vec<u8> {
    encode(request, &searching(), &texts(), &BTreeMap::new())
        .expect("不要 blob")
        .body
}

/// 请求体里 `input` 每一项的原文。
fn input_items(body: &[u8]) -> Vec<String> {
    #[derive(Deserialize)]
    struct Body<'a> {
        #[serde(borrow)]
        input: Vec<&'a RawValue>,
    }
    let parsed: Body<'_> = serde_json::from_slice(body).expect("是 JSON");
    parsed
        .input
        .iter()
        .map(|item| item.get().to_string())
        .collect()
}

/// 真的那一段流解出来的六项，下一次请求里写成那段字，接进上一轮的正文（2026-11-19 实测：原样带回的 DeepSeek 不放进
/// 上下文，模型不记得自己搜过）；搜了哪几句、开了哪几页都在字里。留着历史的编码和整份编码一字不差。
#[test]
fn decoded_searches_are_written_back_as_text() {
    let stream = sample();
    let request = request(reply(&stream));
    let body = whole(&request);
    let items = input_items(&body);
    // 用户的一句、上一轮的回答（六段字和正文合成一项）、这一轮用户的一句。
    assert_eq!(items.len(), 3);
    let said: Value = serde_json::from_str(&items[1]).expect("是 JSON");
    let content = said["content"].as_str().expect("是字");
    let told = |name: &str| {
        content
            .matches(&format!("<hosted-tool name=\"{name}\">"))
            .count()
    };
    assert_eq!(
        (told("deepseek_web_search"), told("deepseek_read_url")),
        (2, 4)
    );
    for known in [
        "latest stable Rust version release date 2025",
        "https://blog.rust-lang.org/releases/",
        "unavailable",
    ] {
        assert!(content.contains(known), "{known}");
    }
    assert!(
        content.ends_with("released on **October 15, 2026**."),
        "{content}"
    );
    for item in wire_items(&stream, "response.output_item.done") {
        assert!(!items.contains(&item), "没有原样带回");
    }
    let mut memo = Memo::new();
    let none = BTreeMap::new();
    let call = searching();
    let kept = |request: &Request, memo: &mut Memo| {
        fragments_memo(request, &call, &texts(), &none, memo)
            .expect("不要 blob")
            .to_bytes(&none)
            .expect("拼得起来")
            .body
    };
    assert_eq!(kept(&request, &mut memo), body);
    let mut longer = request.clone();
    longer.messages.push(Message::Assistant {
        blocks: vec![text("不客气。")],
    });
    longer.messages.push(Message::User {
        blocks: vec![text("再见")],
    });
    assert_eq!(kept(&longer, &mut memo), whole(&longer));
}

/// 一段只有一项 `item` 的流：先 `added`（`added` 是假的只发 `done`），再 `done`。
fn one_item(item: &Value, added: bool) -> Vec<u8> {
    let mut stream = String::new();
    if added {
        let mut opening = item.clone();
        opening["status"] = "in_progress".into();
        let data = serde_json::json!({"type":"response.output_item.added","output_index":0,"item":opening});
        stream.push_str(&format!(
            "event: response.output_item.added\ndata: {data}\n\n"
        ));
    }
    let data = serde_json::json!({"type":"response.output_item.done","output_index":0,"item":item});
    stream.push_str(&format!(
        "event: response.output_item.done\ndata: {data}\n\n"
    ));
    stream.push_str("event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"server_tool_use\":{\"tool_search\":0,\"tool_fetch\":0}}}}\n\n");
    stream.into_bytes()
}

fn only(stream: &[u8]) -> HostedTool {
    let blocks = reply(stream);
    match &blocks[..] {
        [Block::HostedTool(hosted)] => hosted.clone(),
        other => panic!("只有一块供应商侧工具：{other:?}"),
    }
}

#[test]
fn names_follow_the_action_and_pages_say_url_and_title() {
    // 没有 `kind` 的是 OpenAI 自己的 `web_search`。
    let openai = serde_json::json!({"type":"web_search_call","id":"ws_1","status":"completed","action":{"type":"search","query":"rust"}});
    let block = only(&one_item(&openai, true));
    assert_eq!(block.name, "web_search");
    // 参数是流里 `action` 的原文（这里流是 `json!` 写的，键的先后随它）。
    assert_eq!(block.args, openai["action"].to_string());
    // 没有 `sources` 的搜索：结果是整项原文。
    assert_eq!(block.outcome.expect("有").text, openai.to_string());
    // 打开网页的是 `deepseek_read_url`（读链接的、深度搜索里的都是）；只发了 `done` 的也开块。读链接的项带着的
    // `signature`（加密的网页正文）不存。
    let read = serde_json::json!({"type":"web_search_call","id":"ws_2","status":"completed","kind":"read_link","action":{"type":"open_page","url":"https://a.example/"},"page":{"url":"https://a.example/final","title":"A"},"signature":"c2ln"});
    let block = only(&one_item(&read, false));
    assert_eq!(block.name, "deepseek_read_url");
    assert!(block.private.is_none());
    assert_eq!(
        block.outcome.expect("有").text,
        "https://a.example/final\nA"
    );
    let find = serde_json::json!({"type":"web_search_call","id":"ws_3","status":"completed","kind":"deep_search","action":{"type":"find_in_page","url":"https://a.example/","pattern":"1.99"}});
    let block = only(&one_item(&find, true));
    assert_eq!(block.name, "deepseek_read_url");
    assert_eq!(
        block.outcome.expect("有").text,
        "https://a.example/\npattern: 1.99"
    );
    let image = serde_json::json!({"type":"web_search_call","id":"ws_4","status":"failed","kind":"image_search","action":{"type":"search","queries":["cat"]}});
    let block = only(&one_item(&image, true));
    assert_eq!(block.name, "deepseek_web_search");
    let outcome = block.outcome.expect("有");
    assert!(outcome.failed, "失败了、没有错误码的写整项原文");
    assert_eq!(outcome.text, image.to_string());
}
