//! 统一请求的测试：同样的请求字节一样；参数格式原样照抄；指纹说得出第一处不同在哪，
//! 只在后面接着加的没有不同，少了的从少了的那一条算。

use super::*;
use crate::block::{Text, ToolCall};

fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

fn call() -> CallId {
    CallId::parse("call_2_1").unwrap()
}

/// 一份三条消息的请求：人说一句，模型调一个工具，工具的结果。
fn request() -> Request {
    Request {
        tools: vec![ToolSpec {
            name: "read".to_string(),
            description: "Read a file.".to_string(),
            parameters: serde_json::from_str(r#"{"type":"object"}"#).unwrap(),
        }],
        system: "You are a helpful software engineer.".to_string(),
        messages: vec![
            Message::User {
                blocks: vec![text("看看 src 目录")],
            },
            Message::Assistant {
                blocks: vec![Block::ToolCall(ToolCall {
                    call_id: call(),
                    name: "read".to_string(),
                    args: r#"{"path":"src"}"#.to_string(),
                    private: None,
                })],
            },
            Message::Tool {
                call_id: call(),
                error: false,
                blocks: vec![text("lib.rs")],
            },
        ],
        stable: 0,
        continuation: false,
        described: Default::default(),
        full_parameters: Default::default(),
        output_cap: None,
        used: None,
    }
}

#[test]
fn the_same_request_has_the_same_bytes_and_hash() {
    assert_eq!(request().canonical_bytes(), request().canonical_bytes());
    assert_eq!(
        request().hash(),
        ContentHash::of(&request().canonical_bytes())
    );
}

/// 参数格式原样照抄，连空格都不动：重新写一遍会改变字节。
#[test]
fn tool_parameters_are_kept_byte_for_byte() {
    let mut request = request();
    request.tools[0].parameters = serde_json::from_str(r#"{ "type" : "object" }"#).unwrap();
    let bytes = String::from_utf8(request.canonical_bytes()).unwrap();
    assert!(
        bytes.contains(r#""parameters":{ "type" : "object" }"#),
        "{bytes}"
    );
}

/// 每条消息写出来都以角色开头，角色的写法和 `Role::as_str` 一样。
#[test]
fn every_message_is_written_role_first() {
    for message in request().messages {
        let bytes = String::from_utf8(json(&message)).unwrap();
        let head = format!(r#"{{"role":"{}","#, message.role().as_str());
        assert!(bytes.starts_with(&head), "{bytes}");
    }
}

#[test]
fn a_request_that_only_grows_has_no_difference() {
    let before = request().fingerprint();
    assert_eq!(request().fingerprint().first_difference(&before), None);
    let mut longer = request();
    longer.messages.push(Message::User {
        blocks: vec![text("只看 .rs 文件")],
    });
    assert_eq!(longer.fingerprint().first_difference(&before), None);
}

#[test]
fn tools_and_system_are_compared_first() {
    let before = request().fingerprint();
    let mut changed = request();
    changed.tools[0].description = "Read a file or a directory.".to_string();
    changed.messages.clear();
    assert_eq!(
        changed.fingerprint().first_difference(&before),
        Some(Difference::Tools)
    );
    let mut changed = request();
    changed.system.push_str(" Answer briefly.");
    assert_eq!(
        changed.fingerprint().first_difference(&before),
        Some(Difference::System)
    );
}

#[test]
fn a_changed_message_is_reported_with_its_index_and_role() {
    let before = request().fingerprint();
    let mut changed = request();
    changed.messages[2] = Message::Tool {
        call_id: call(),
        error: true,
        blocks: vec![text("lib.rs")],
    };
    assert_eq!(
        changed.fingerprint().first_difference(&before),
        Some(Difference::Message {
            index: 2,
            role: Role::Tool
        })
    );
}

#[test]
fn a_missing_message_counts_from_where_it_went_missing() {
    let before = request().fingerprint();
    let mut shorter = request();
    shorter.messages.truncate(1);
    assert_eq!(
        shorter.fingerprint().first_difference(&before),
        Some(Difference::Message {
            index: 1,
            role: Role::Assistant
        })
    );
}

/// 图的转述（施工 8-17）：空的不写进规范字节，以前的请求字节、哈希不变；有的写在最后，键是 blob；不算进指纹。
#[test]
fn descriptions_are_written_last_only_when_there_are_some() {
    let plain = String::from_utf8(request().canonical_bytes()).unwrap();
    assert!(!plain.contains("described"), "{plain}");
    assert!(plain.ends_with(r#""stable":0}"#), "{plain}");
    let blob = ContentHash::of(b"png");
    let mut described = request();
    described
        .described
        .insert(blob.clone(), "A red \"sign\".".to_string());
    let bytes = String::from_utf8(described.canonical_bytes()).unwrap();
    assert_eq!(
        bytes,
        format!(
            r#"{},"described":{{"{blob}":"A red \"sign\"."}}}}"#,
            plain.strip_suffix('}').unwrap()
        )
    );
    assert_ne!(described.hash(), request().hash());
    assert_eq!(
        described
            .fingerprint()
            .first_difference(&request().fingerprint()),
        None,
        "转述不算进指纹"
    );
}

/// 要先加载的工具的完整参数格式（施工 L-2）：空的不写进规范字节，以前的请求字节、哈希不变；有的原样照抄写在最后；不算进指纹。
#[test]
fn full_parameters_are_written_last_only_when_there_are_some() {
    let plain = String::from_utf8(request().canonical_bytes()).unwrap();
    assert!(!plain.contains("full_parameters"), "{plain}");
    let mut full = request();
    full.full_parameters.insert(
        "lookup".to_string(),
        serde_json::from_str(r#"{"type":"object", "required":["word"]}"#).unwrap(),
    );
    let bytes = String::from_utf8(full.canonical_bytes()).unwrap();
    assert_eq!(
        bytes,
        format!(
            r#"{},"full_parameters":{{"lookup":{{"type":"object", "required":["word"]}}}}}}"#,
            plain.strip_suffix('}').unwrap()
        )
    );
    assert_ne!(full.hash(), request().hash());
    assert_eq!(
        full.fingerprint()
            .first_difference(&request().fingerprint()),
        None,
        "完整格式不算进指纹"
    );
}

/// 指纹照结构算（施工 V-12：一万条事件的请求，原来每条消息写成 JSON 再算 SHA-256，是回合热路径上最大的一截）：原来写进
/// JSON 的都照样比得出来——工具的参数格式、调用里驱动的私有数据。
#[test]
fn the_fingerprint_still_sees_what_the_bytes_carried() {
    let before = request().fingerprint();
    let mut schema = request();
    schema.tools[0].parameters =
        serde_json::from_str(r#"{"type":"object","required":[]}"#).unwrap();
    assert_eq!(
        schema.fingerprint().first_difference(&before),
        Some(Difference::Tools)
    );
    let mut private = request();
    let Message::Assistant { blocks } = &mut private.messages[1] else {
        panic!("第二条是回复");
    };
    let Block::ToolCall(call) = &mut blocks[0] else {
        panic!("回复里是调用");
    };
    call.private = Some(crate::block::Private {
        driver: crate::id::DriverFamily::parse("anthropic").unwrap(),
        data: serde_json::from_str(r#"{"signature":"abc"}"#).unwrap(),
    });
    assert_eq!(
        private.fingerprint().first_difference(&before),
        Some(Difference::Message {
            index: 1,
            role: Role::Assistant
        })
    );
    let mut error = request();
    let Message::Tool { error: failed, .. } = &mut error.messages[2] else {
        panic!("第三条是结果");
    };
    *failed = true;
    assert_eq!(
        error.fingerprint().first_difference(&before),
        Some(Difference::Message {
            index: 2,
            role: Role::Tool
        })
    );
}

/// 指纹换成一次吃 8 个字节的快哈希以后（施工 V-17 下）：只差末尾补零、只差长度、凑满 8 字节前后的几条照样分得出。
#[test]
fn near_identical_texts_still_differ() {
    let texts = [
        "ab",
        "ab\0",
        "ab\0\0",
        "abcdefgh",
        "abcdefgh\0",
        "abcdefg",
        "",
        "\0",
    ];
    for (i, one) in texts.iter().enumerate() {
        for other in &texts[i + 1..] {
            let mut before = request();
            before.messages[0] = Message::User {
                blocks: vec![text(one)],
            };
            let mut after = request();
            after.messages[0] = Message::User {
                blocks: vec![text(other)],
            };
            assert_eq!(
                after.fingerprint().first_difference(&before.fingerprint()),
                Some(Difference::Message {
                    index: 0,
                    role: Role::User
                }),
                "{one:?} 和 {other:?}"
            );
        }
    }
}
