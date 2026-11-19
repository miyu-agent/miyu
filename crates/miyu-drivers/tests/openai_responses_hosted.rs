//! 供应商那边的工具声明（`Call.hosted`，设计 72，施工 DSR-3）：openai-responses 把它们照先后接在函数工具后面，没有函数工具的也写
//! `tools`；留着历史的编码和整份编码一个字节不差，前后两次的 `hosted` 不一样时各是各的；空的和以前一样；别的驱动不认它。

use std::collections::BTreeMap;

use crate::support::{claude, deepseek, gpt, id, read_tool, text, texts, tool_call};
use miyu_drivers::openai_responses::{encode, fragments_memo};
use miyu_drivers::{Anthropic, Call, Driver, Inputs, Memo, OpenAiChat, OpenAiResponses};
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};

/// 两件供应商那边的工具声明：DeepSeek 官方 Responses 接口的两个名字（出厂的档案只声明前一件，这里用两件看先后）。
fn deepseek_hosted() -> Vec<String> {
    vec![
        "deepseek_web_search".to_string(),
        "deepseek_read_url".to_string(),
    ]
}

fn request(tools: bool, messages: Vec<Message>) -> Request {
    Request {
        tools: if tools { vec![read_tool()] } else { vec![] },
        system: "You are a helpful assistant.".to_string(),
        messages,
        stable: 0,
        continuation: false,
        described: Default::default(),
        output_cap: None,
        used: None,
        tail: None,
    }
}

fn said(words: &str) -> Message {
    Message::User {
        blocks: vec![text(words)],
    }
}

/// 发给 `gpt-5.4`，接上 `hosted`。
fn with(mut call: Call, hosted: &[String]) -> Call {
    call.hosted = hosted.to_vec();
    call
}

fn whole(request: &Request, call: &Call) -> Vec<u8> {
    encode(request, call, &texts(), &BTreeMap::new())
        .expect("不要 blob 的请求编码得了")
        .body
}

fn kept(request: &Request, call: &Call, memo: &mut Memo) -> Vec<u8> {
    let none = BTreeMap::new();
    fragments_memo(request, call, &texts(), &none, memo)
        .expect("不要 blob 的请求编码得了")
        .to_bytes(&none)
        .expect("拼得起来")
        .body
}

fn tools(body: &[u8]) -> Value {
    serde_json::from_slice::<Value>(body).expect("请求字节是 JSON")["tools"].clone()
}

#[test]
fn hosted_declarations_follow_the_function_tools_in_order() {
    let call = with(gpt(Inputs::default(), None), &deepseek_hosted());
    let body = whole(&request(true, vec![said("今天的新闻")]), &call);
    let text = String::from_utf8(body.clone()).expect("UTF-8");
    assert!(
        text.contains(
            r#""strict":false},{"type":"deepseek_web_search"},{"type":"deepseek_read_url"}],"store":false"#
        ),
        "{text}"
    );
    let tools = tools(&body);
    assert_eq!(tools.as_array().map(Vec::len), Some(3));
    assert_eq!(tools[0]["type"], "function");
}

#[test]
fn hosted_declarations_alone_still_write_the_tools() {
    let call = with(gpt(Inputs::default(), None), &deepseek_hosted());
    let body = whole(&request(false, vec![said("今天的新闻")]), &call);
    let written = String::from_utf8(body).expect("UTF-8");
    assert!(
        written.contains(
            r#"],"tools":[{"type":"deepseek_web_search"},{"type":"deepseek_read_url"}],"store":false"#
        ),
        "{written}"
    );
    // 历史里有调用、没有函数工具的：空的那一截后面照样接。
    let called = request(
        false,
        vec![
            said("看看"),
            Message::Assistant {
                blocks: vec![tool_call("call_1_1", "read", "{}", None)],
            },
            Message::Tool {
                call_id: id("call_1_1"),
                error: false,
                blocks: vec![text("ok")],
            },
        ],
    );
    assert_eq!(
        tools(&whole(&called, &call)),
        json!([{"type": "deepseek_web_search"}, {"type": "deepseek_read_url"}])
    );
}

/// 名字照 JSON 的规矩转义：写不出别的字段。
#[test]
fn hosted_names_are_escaped() {
    let call = with(gpt(Inputs::default(), None), &["a\"b\\c".to_string()]);
    let body = whole(&request(false, vec![said("hi")]), &call);
    assert_eq!(tools(&body), json!([{"type": "a\"b\\c"}]));
}

/// 留着历史的编码（施工 V-42）和整份编码一个字节不差；工具面留着的那一截不带供应商的声明，`hosted` 前后两次不一样时各编各的。
#[test]
fn kept_history_matches_the_whole_encoding_whatever_hosted_is() {
    let mut memo = Memo::new();
    let plain = gpt(Inputs::default(), None);
    let searching = with(plain.clone(), &deepseek_hosted());
    let only_search = with(plain.clone(), &deepseek_hosted()[..1]);
    let mut messages = vec![said("第一句")];
    for (round, call) in [&searching, &plain, &searching, &only_search, &plain]
        .into_iter()
        .enumerate()
    {
        for functions in [true, false] {
            let request = request(functions, messages.clone());
            assert_eq!(
                kept(&request, call, &mut memo),
                whole(&request, call),
                "第 {round} 轮，函数工具 {functions}，hosted {:?}",
                call.hosted
            );
        }
        messages.push(Message::Assistant {
            blocks: vec![text("好")],
        });
        messages.push(said("再来"));
    }
    // 空的和以前一样：不写供应商那一截，没有函数工具、没有调用的不发 `tools`。
    let bare = whole(&request(false, vec![said("hi")]), &plain);
    assert!(!String::from_utf8(bare).expect("UTF-8").contains("tools"));
}

/// 别的驱动不认 `hosted`：有没有都编出同样的字节，留着历史的也一样。
#[test]
fn other_drivers_ignore_hosted() {
    let request = request(true, vec![said("今天的新闻")]);
    let chat = OpenAiChat::new(deepseek(), texts());
    let anthropic = Anthropic::new(texts());
    let drivers: [(&dyn Driver, Call); 2] = [
        (&chat, crate::support::call(Inputs::default(), None)),
        (&anthropic, claude(Inputs::default(), Some(1024))),
    ];
    let none = BTreeMap::new();
    for (driver, call) in drivers {
        let searching = with(call.clone(), &deepseek_hosted());
        let bytes = |call: &Call| driver.encode(&request, call, &none).expect("编码得了").body;
        assert_eq!(bytes(&searching), bytes(&call), "{}", driver.family());
        let mut memo = Memo::new();
        let mut kept = |call: &Call| {
            driver
                .fragments_memo(&request, call, &none, &mut memo)
                .expect("编码得了")
                .to_bytes(&none)
                .expect("拼得起来")
                .body
        };
        assert_eq!(kept(&searching), kept(&call), "{}", driver.family());
    }
    // openai-responses 走驱动的接口也接上了。
    let responses = OpenAiResponses::new(texts());
    let call = with(gpt(Inputs::default(), None), &deepseek_hosted());
    let body = responses
        .encode(&request, &call, &none)
        .expect("编码得了")
        .body;
    assert_eq!(tools(&body)[2], json!({"type": "deepseek_read_url"}));
}
