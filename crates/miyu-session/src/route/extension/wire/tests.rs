//! 扩展驱动线上的写法（施工 MD-2）：参数的样子、增量和回答怎么读、分类怎么认、原话截多长、图怎么挑。

use serde_json::json;

use miyu_drivers::Inputs;
use miyu_kernel::block::{HostedTool, Image, Text};
use miyu_kernel::id::CallId;
use miyu_kernel::id::{MediaType, ModelName};

use super::*;

fn family() -> DriverFamily {
    DriverFamily::parse("pi/ag").expect("家族")
}

#[test]
fn params_put_the_request_first_and_skip_what_is_not_there() {
    let request = Request {
        tools: Vec::new(),
        system: "s".to_string(),
        messages: Vec::new(),
        stable: 0,
        continuation: false,
        described: BTreeMap::new(),
        output_cap: None,
        used: None,
        tail: None,
    };
    let bytes = request.canonical_bytes();
    let call = Call {
        model: ModelName::parse("m").expect("模型名"),
        max_output: None,
        inputs: Inputs::default(),
        effort: Some("high".to_string()),
        temperature: Some(0.5),
    };
    let blobs = BTreeMap::new();
    let fingerprint = request.fingerprint();
    let outgoing = |key, base_url| Outgoing {
        call: "d1",
        driver: "pi/ag",
        request: &bytes,
        fingerprint: &fingerprint,
        params: &call,
        key,
        base_url,
        blobs: &blobs,
    };
    let text = params(&outgoing(None, None));
    assert!(text.starts_with(&format!(
        "{{\"request\":{},",
        String::from_utf8_lossy(&bytes)
    )));
    let value: Value = serde_json::from_str(&text).expect("是 JSON");
    assert_eq!(value["call"], "d1");
    assert_eq!(
        value["params"],
        json!({"model": "m", "effort": "high", "temperature": 0.5})
    );
    for absent in ["key", "base_url", "blobs"] {
        assert!(value.get(absent).is_none(), "{absent}");
    }
    let value: Value =
        serde_json::from_str(&params(&outgoing(Some("sk"), Some("https://a.invalid"))))
            .expect("JSON");
    assert_eq!(
        (value["key"].as_str(), value["base_url"].as_str()),
        (Some("sk"), Some("https://a.invalid"))
    );
}

#[test]
fn every_delta_kind_reads() {
    let read = deltas(
        json!([
            {"start": {"index": 0, "kind": "text"}},
            {"start": {"index": 1, "kind": {"tool_call": {"name": "read"}}}},
            {"text": {"index": 1, "text": "{}"}},
            {"private": {"index": 1, "data": "call_1"}},
            {"end": {"index": 1}},
            "truncated"
        ]),
        &family(),
    )
    .expect("读得出");
    assert_eq!(
        read[1],
        Delta::Start {
            index: 1,
            kind: Kind::ToolCall {
                name: "read".to_string()
            }
        }
    );
    let Delta::Private { private, .. } = &read[3] else {
        panic!("{read:?}")
    };
    assert_eq!(
        (private.driver.as_str(), private.data.get()),
        ("pi/ag", "\"call_1\"")
    );
    assert_eq!(read[5], Delta::Truncated);
    for bad in [
        json!({"start": {"index": 0, "kind": "text"}}),
        json!([{"start": {"index": 0, "kind": "image"}}]),
        json!([{"text": {"index": 0}}]),
        json!([{"end": {"index": 0, "extra": 1}}]),
        json!(["finished"]),
    ] {
        assert!(deltas(bad.clone(), &family()).is_err(), "{bad}");
    }
}

#[test]
fn errors_are_classified_cut_and_only_rate_limits_wait() {
    for (class, wanted) in [
        ("retryable", ErrorClass::Retryable),
        ("rate_limited", ErrorClass::RateLimited),
        ("context_too_long", ErrorClass::ContextTooLong),
        ("auth", ErrorClass::Auth),
        ("quota", ErrorClass::Quota),
        ("content_policy", ErrorClass::ContentPolicy),
        ("unclassified", ErrorClass::Unclassified),
        ("bad_stream", ErrorClass::Unclassified),
        ("no_model", ErrorClass::Unclassified),
    ] {
        let said =
            answer(json!({"error": {"class": class, "message": "m", "retry_after_ms": 900}}))
                .expect("读得出");
        let error = said.error.expect("出错").error;
        assert_eq!(error.class, wanted, "{class}");
        let waits = (wanted == ErrorClass::RateLimited).then_some(900);
        assert_eq!(error.retry_after_ms, waits, "{class}");
    }
    let long = "字".repeat(800);
    let said = answer(json!({"error": {"class": "auth", "message": long}})).expect("读得出");
    assert_eq!(said.error.expect("出错").error.message.chars().count(), 500);
    let said = answer(json!({})).expect("什么都不报也行");
    assert!(said.usage.is_none() && said.error.is_none() && said.sent.is_none());
    for bad in [
        json!({"sent": "not-a-hash"}),
        json!({"usage": {"uncached": "1"}}),
        json!({"error": {"class": "auth"}}),
        json!({"cost": 1}),
        json!([]),
    ] {
        assert!(answer(bad.clone()).is_err(), "{bad}");
    }
}

#[test]
fn images_are_listed_once_with_their_type() {
    let image = |hash: &str| {
        Block::Image(Image {
            blob: ContentHash::parse(&format!("sha256:{}", hash.repeat(64))).expect("哈希"),
            name: None,
            media_type: MediaType::parse("image/png").expect("类型"),
            width: 1,
            height: 1,
            path: None,
        })
    };
    let request = Request {
        tools: Vec::new(),
        system: String::new(),
        messages: vec![
            Message::User {
                blocks: vec![
                    image("a"),
                    Block::Text(Text {
                        text: "x".to_string(),
                    }),
                ],
            },
            Message::User {
                blocks: vec![image("a"), image("b")],
            },
        ],
        stable: 0,
        continuation: false,
        described: BTreeMap::new(),
        output_cap: None,
        used: None,
        tail: None,
    };
    let listed = images(&request);
    assert_eq!(listed.len(), 2);
    assert!(listed.values().all(|media_type| media_type == "image/png"));
}

/// 供应商侧工具（施工 HT-1）带的私有数据同工具调用：别家的认得出、拿掉，自己家的留着。
#[test]
fn a_hosted_tool_s_foreign_private_data_is_stripped() {
    let hosted = |driver: &str| {
        Block::HostedTool(HostedTool {
            call_id: CallId::parse("call_3_1").expect("编号"),
            name: "web_search".to_string(),
            args: "{}".to_string(),
            outcome: None,
            private: Some(Private {
                driver: DriverFamily::parse(driver).expect("家族"),
                data: RawJson::of(&json!("ws_1")).expect("JSON"),
            }),
        })
    };
    let request = |driver: &str| Request {
        tools: Vec::new(),
        system: String::new(),
        messages: vec![Message::Assistant {
            blocks: vec![hosted(driver)],
        }],
        stable: 0,
        continuation: false,
        described: BTreeMap::new(),
        output_cap: None,
        used: None,
        tail: None,
    };
    let own = request("pi/ag");
    assert!(!foreign(&own, &family()));
    let mut theirs = request("anthropic");
    assert!(foreign(&theirs, &family()));
    strip(&mut theirs, &family());
    let Message::Assistant { blocks } = &theirs.messages[0] else {
        panic!("助手消息");
    };
    let [Block::HostedTool(stripped)] = blocks.as_slice() else {
        panic!("块还在");
    };
    assert_eq!(stripped.private, None);
}
