//! 约束解码的那一家（施工 L-2，`docs/designs/25-工具加载.md` W9）：要先加载的工具在工具面上是外壳，标了 `strict_tools` 的
//! 照请求的 `full_parameters` 发完整格式，没标的照外壳发；不在 `full_parameters` 里的照工具面原样；别的字节都一样。

use std::collections::BTreeMap;

use crate::support::{call, raw, read_tool, sees_all, text, texts};
use miyu_drivers::openai_chat::{Compat, encode};
use miyu_kernel::request::{Message, Request, ToolSpec};
use serde_json::Value;

const FULL: &str =
    r#"{"type":"object","properties":{"word":{"type":"string"}},"required":["word"]}"#;

/// 工具面：`lookup` 是外壳，`read` 全量；`lookup` 的完整格式在 `full_parameters` 里。
fn request() -> Request {
    let lookup = ToolSpec {
        name: "lookup".to_string(),
        description: "Look a word up.".to_string(),
        parameters: raw(r#"{"type":"object"}"#),
    };
    Request {
        tools: vec![lookup, read_tool()],
        system: "You are a helpful assistant.".to_string(),
        messages: vec![Message::User {
            blocks: vec![text("cat 是什么意思？")],
        }],
        stable: 0,
        continuation: false,
        described: BTreeMap::new(),
        full_parameters: [("lookup".to_string(), raw(FULL))].into(),
        output_cap: None,
        used: None,
    }
}

fn bytes(request: &Request, compat: &Compat) -> Vec<u8> {
    encode(
        request,
        &call(sees_all(), None),
        compat,
        &texts(),
        &BTreeMap::new(),
    )
    .expect("没有要取的 blob")
    .body
}

fn body(request: &Request, compat: &Compat) -> Value {
    serde_json::from_slice(&bytes(request, compat)).expect("是 JSON")
}

fn parameters(body: &Value, n: usize) -> Value {
    body["tools"][n]["function"]["parameters"].clone()
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("是 JSON")
}

#[test]
fn a_strict_provider_gets_the_full_parameters() {
    let strict = Compat {
        strict_tools: true,
        ..Compat::default()
    };
    let sent = bytes(&request(), &strict);
    let wire = String::from_utf8(sent.clone()).expect("是 UTF-8");
    assert!(
        wire.contains(&format!("\"parameters\":{FULL}")),
        "原样照抄：{wire}"
    );
    let body: Value = serde_json::from_slice(&sent).expect("是 JSON");
    assert_eq!(body["tools"][0]["function"]["name"], "lookup");
    assert_eq!(parameters(&body, 0), json(FULL), "换回完整格式");
    assert_eq!(
        parameters(&body, 1),
        json(read_tool().parameters.get()),
        "不用加载的照原样"
    );
}

#[test]
fn other_providers_get_the_shell() {
    let body = body(&request(), &Compat::default());
    assert_eq!(parameters(&body, 0), json(r#"{"type":"object"}"#));
}

#[test]
fn only_the_tools_differ() {
    let strict = Compat {
        strict_tools: true,
        ..Compat::default()
    };
    let mut full = body(&request(), &strict);
    let mut shell = body(&request(), &Compat::default());
    full["tools"] = Value::Null;
    shell["tools"] = Value::Null;
    assert_eq!(full, shell);
}

#[test]
fn without_full_parameters_strict_changes_nothing() {
    let mut plain = request();
    plain.full_parameters.clear();
    let strict = Compat {
        strict_tools: true,
        ..Compat::default()
    };
    assert_eq!(bytes(&plain, &strict), bytes(&plain, &Compat::default()));
}
