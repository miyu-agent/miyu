//! Anthropic 的服务端工具（`docs/blueprint/drivers/anthropic.md`「怎么走：解码」第 6 条、「怎么走：编码」第 5 条，施工 HT-5）：
//! 私有数据是 `use`、`result` 两块原文，超过上限的、参数坏了的不交；拼好的块带着这一家的私有数据；编码时照原样写回两块，
//! 和流里的逐字节相同，别家的、读不懂的、没带的照 HT-2 写成字；稳定区的打点落在它后面时不乱。流的样本由
//! `anthropic_streams.rs` 比。

use std::collections::BTreeMap;
use std::fs;

use crate::support::{anthropic_sample, claude, family_dir, id, private, sees_all, text, texts};
use miyu_drivers::anthropic::{Decoder, PRIVATE_BYTES, encode};
use miyu_kernel::accumulate::{Accumulator, Delta};
use miyu_kernel::block::{Block, HostedTool, Outcome, Private};
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};

/// 带服务端工具的流的样本：正文、搜索、结果两条、正文。
fn search_stream() -> String {
    let path = family_dir("anthropic").join("streams/server-tool-search.sse");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()))
}

/// 流里 `content_block` 的种类是 `kind` 的那一条事件里，`content_block` 那一格的原文：从事件里照字切出来，不经过解析。
fn wire_block(stream: &str, kind: &str) -> String {
    let line = stream
        .lines()
        .find(|line| line.contains(&format!(r#""content_block":{{"type":"{kind}""#)))
        .expect("流里有这一块");
    let start = line.find(r#""content_block":"#).expect("有这一格") + r#""content_block":"#.len();
    // 这一格是事件的最后一格：去掉事件自己收尾的 `}`。
    line[start..line.len() - 1].to_string()
}

/// 一片喂完，收尾，交出全部增量。
fn deltas(stream: &[u8]) -> Vec<Delta> {
    let mut decoder = Decoder::new();
    let mut out = decoder.feed(stream);
    let ending = decoder.finish();
    assert!(ending.error.is_none(), "{:?}", ending.error);
    out.extend(ending.deltas);
    out
}

/// 拼好的回复。
fn reply(stream: &[u8]) -> Vec<Block> {
    let mut accumulator = Accumulator::default();
    for delta in deltas(stream) {
        accumulator.apply(delta).expect("累积器收得下");
    }
    accumulator.finish(Seq::FIRST)
}

/// 交出的私有数据。
fn privates(deltas: &[Delta]) -> Vec<&Private> {
    deltas
        .iter()
        .filter_map(|delta| match delta {
            Delta::Private { private, .. } => Some(private),
            _ => None,
        })
        .collect()
}

/// 交出的结果。
fn outcomes(deltas: &[Delta]) -> Vec<(&str, bool)> {
    deltas
        .iter()
        .filter_map(|delta| match delta {
            Delta::Outcome { text, failed, .. } => Some((text.as_str(), *failed)),
            _ => None,
        })
        .collect()
}

/// 一段只有一次抓取的流：参数原文是 `args`，抓到的正文是 `data`。
fn fetch_stream(args: &str, data: &str) -> Vec<u8> {
    let content = json!({
        "type": "web_fetch_result",
        "url": "https://example.com/big",
        "content": {"type": "document", "source": {"type": "text", "media_type": "text/plain", "data": data}},
    });
    let result =
        json!({"type": "web_fetch_tool_result", "tool_use_id": "srvtoolu_big", "content": content});
    let args = serde_json::to_string(args).expect("字符串写得成 JSON");
    format!(
        "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"usage\":{{\"input_tokens\":1}}}}}}\n\n\
         event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"server_tool_use\",\"id\":\"srvtoolu_big\",\"name\":\"web_fetch\",\"input\":{{}}}}}}\n\n\
         event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"input_json_delta\",\"partial_json\":{args}}}}}\n\n\
         event: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n\
         event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{result}}}\n\n\
         event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"output_tokens\":2}}}}\n\n\
         event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n"
    )
    .into_bytes()
}

#[test]
fn the_private_data_is_the_use_and_the_result_as_they_came() {
    let stream = search_stream();
    let deltas = deltas(stream.as_bytes());
    let privates = privates(&deltas);
    assert_eq!(privates.len(), 1);
    assert_eq!(privates[0].driver.as_str(), "anthropic");
    // `use`：起始块的每一格照原来的先后，`input` 换成收全的参数原文，别的格（`caller`）一格不丢。
    let used = r#"{"type":"server_tool_use","id":"srvtoolu_01","name":"web_search","input":{"query":"rust 1.97"},"caller":{"type":"direct"}}"#;
    // `result`：流里那一格的原文，一个字节不改。
    let result = wire_block(&stream, "web_search_tool_result");
    assert!(result.contains("encrypted_content"));
    assert_eq!(
        privates[0].data.get(),
        format!(r#"{{"use":{used},"result":{result}}}"#)
    );
}

#[test]
fn the_decoded_block_carries_the_outcome_and_this_familys_private_data() {
    let blocks = reply(search_stream().as_bytes());
    assert_eq!(
        blocks.len(),
        3,
        "正文、供应商侧工具、正文：结果块不另成一块"
    );
    let Block::HostedTool(hosted) = &blocks[1] else {
        panic!("第二块是供应商侧工具：{:?}", blocks[1]);
    };
    assert_eq!(hosted.name, "web_search");
    assert_eq!(hosted.args, r#"{"query":"rust 1.97"}"#);
    assert_eq!(hosted.call_id.to_string(), "call_1_1");
    assert_eq!(
        hosted.outcome,
        Some(Outcome {
            text: "Announcing Rust 1.97.0 - https://blog.rust-lang.org/2026/07/07/Rust-1.97.0/\n\
                   Rust 1.97 发布说明 - https://example.com/rust?a=1&b=2"
                .to_string(),
            failed: false,
            cut: false,
        })
    );
    let private = hosted.private.as_ref().expect("带着私有数据");
    assert_eq!(private.driver.as_str(), "anthropic");
}

#[test]
fn a_private_data_over_the_cap_is_left_out_but_the_outcome_stays() {
    let measured = deltas(&fetch_stream(r#"{"url":"https://example.com/big"}"#, ""));
    let base = privates(&measured)[0].data.get().len();
    // 正好到上限的交；多一个字节的不交，结果照交、是全文。
    let fits = deltas(&fetch_stream(
        r#"{"url":"https://example.com/big"}"#,
        &"a".repeat(PRIVATE_BYTES - base),
    ));
    assert_eq!(privates(&fits)[0].data.get().len(), PRIVATE_BYTES);
    let over = deltas(&fetch_stream(
        r#"{"url":"https://example.com/big"}"#,
        &"a".repeat(PRIVATE_BYTES - base + 1),
    ));
    assert!(privates(&over).is_empty());
    let outcomes = outcomes(&over);
    assert_eq!(outcomes.len(), 1);
    assert!(
        outcomes[0]
            .0
            .ends_with(&"a".repeat(PRIVATE_BYTES - base + 1))
    );
    assert!(!outcomes[0].1);
    assert!(matches!(over.last(), Some(Delta::End { index: 0 })));
}

#[test]
fn arguments_that_are_not_json_give_no_private_data() {
    let broken = deltas(&fetch_stream(r#"{"url":"https://exam"#, "body"));
    assert!(privates(&broken).is_empty());
    assert_eq!(
        outcomes(&broken),
        vec![("https://example.com/big\n\nbody", false)]
    );
    // 块照样收全，累积器收得下。
    let blocks = reply(&fetch_stream(r#"{"url":"https://exam"#, "body"));
    let Block::HostedTool(hosted) = &blocks[0] else {
        panic!("是供应商侧工具");
    };
    assert_eq!(hosted.args, r#"{"url":"https://exam"#);
    assert!(hosted.private.is_none());
}

fn request(reply: Vec<Block>, stable: usize) -> Request {
    Request {
        tools: vec![],
        system: "You are Miyu.".to_string(),
        messages: vec![
            Message::User {
                blocks: vec![text("Rust 最新是哪一版？")],
            },
            Message::Assistant { blocks: reply },
            Message::User {
                blocks: vec![text("谢谢")],
            },
        ],
        stable,
        continuation: false,
        described: Default::default(),
        output_cap: None,
        used: None,
        tail: None,
    }
}

fn body(request: &Request) -> String {
    let encoded = encode(
        request,
        &claude(sees_all(), None),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob");
    String::from_utf8(encoded.body).expect("UTF-8")
}

fn parsed(body: &str) -> Value {
    serde_json::from_str(&body.replace(r#","cache_control":{"type":"ephemeral"}"#, ""))
        .expect("是 JSON")
}

#[test]
fn decoded_and_encoded_back_the_two_blocks_are_the_stream_blocks() {
    let stream = search_stream();
    let body = body(&request(reply(stream.as_bytes()), 0));
    anthropic_sample("server-tool", body.as_bytes());
    // `result` 和流里的那一格逐字节相同；`use` 是起始块，`input` 换成参数原文。
    let result = wire_block(&stream, "web_search_tool_result");
    let used = wire_block(&stream, "server_tool_use")
        .replace(r#""input":{}"#, r#""input":{"query":"rust 1.97"}"#);
    assert!(
        body.contains(&format!("{used},{result},")),
        "两块原样、挨着：{body}"
    );
    let kinds: Vec<Value> = parsed(&body)["messages"][1]["content"]
        .as_array()
        .expect("是数组")
        .iter()
        .map(|part| part["type"].clone())
        .collect();
    assert_eq!(
        kinds,
        vec![
            json!("text"),
            json!("server_tool_use"),
            json!("web_search_tool_result"),
            json!("text")
        ]
    );
}

/// 一块带着 `private` 的供应商侧工具。
fn hosted(private: Option<Private>) -> Block {
    Block::HostedTool(HostedTool {
        call_id: id("call_1_2"),
        name: "web_search".to_string(),
        args: r#"{"query":"q"}"#.to_string(),
        outcome: Some(Outcome {
            text: "A - https://a.example/".to_string(),
            failed: false,
            cut: false,
        }),
        private,
    })
}

const USE: &str =
    r#"{"type":"server_tool_use","id":"srvtoolu_q","name":"web_search","input":{"query":"q"}}"#;
const RESULT: &str = r#"{"type":"web_search_tool_result","tool_use_id":"srvtoolu_q","content":[{"url":"https://a.example/","type":"web_search_result","title":"A","encrypted_content":"Eq==","n":1.50}]}"#;

#[test]
fn only_this_familys_private_data_of_that_shape_is_written_back() {
    let data = format!(r#"{{"use":{USE},"result":{RESULT}}}"#);
    // 写回：在原来的位置、前后的正文各是各的块，键的先后、数字的写法（`1.50`）都不变。
    let ours = body(&request(
        vec![
            text("我搜一下。"),
            hosted(Some(private("anthropic", &data))),
            text("搜到了。"),
        ],
        0,
    ));
    assert!(
        ours.contains(&format!(
            r#"{{"type":"text","text":"我搜一下。"}},{USE},{RESULT},{{"type":"text","text":"搜到了。"}}"#
        )),
        "{ours}"
    );
    assert!(!ours.contains("<hosted-tool"));
    // 别的都照 HT-2 写成那段字：别家的、读不懂的、不是对象的、没带的。
    let said = parsed(&body(&request(vec![hosted(None)], 0)))["messages"][1]["content"].clone();
    assert_eq!(said[0]["type"], "text");
    assert!(
        said[0]["text"]
            .as_str()
            .expect("是字")
            .starts_with("<hosted-tool")
    );
    for other in [
        private("openai-responses", &data),
        private("anthropic", r#"{"type":"server_tool_use"}"#),
        private("anthropic", &format!(r#"{{"use":{USE}}}"#)),
        private("anthropic", &format!(r#"{{"use":"x","result":{RESULT}}}"#)),
        private("anthropic", &format!(r#"{{"use":{USE},"result":[1]}}"#)),
    ] {
        let data = other.data.get().to_string();
        let got =
            parsed(&body(&request(vec![hosted(Some(other))], 0)))["messages"][1]["content"].clone();
        assert_eq!(got, said, "{data}");
    }
}

#[test]
fn a_stable_mark_after_it_lands_on_the_result_block() {
    let data = format!(r#"{{"use":{USE},"result":{RESULT}}}"#);
    let request = request(
        vec![
            text("我搜一下。"),
            hosted(Some(private("anthropic", &data))),
        ],
        2,
    );
    let body = body(&request);
    let marked = format!(
        r#"{USE},{},"cache_control":{{"type":"ephemeral"}}}}]"#,
        &RESULT[..RESULT.len() - 1]
    );
    assert!(body.contains(&marked), "{body}");
    assert_eq!(
        parsed(&body)["messages"][1]["content"][2],
        serde_json::from_str::<Value>(RESULT).expect("是 JSON")
    );
}
