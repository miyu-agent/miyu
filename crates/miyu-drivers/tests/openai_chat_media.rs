//! OpenAI 兼容对话接口的编码，图片、文件和思考：能收的写成 data URL，不能收的换成占位；工具结果里的
//! 图片、PDF 挪到后面；思考照开关回传，默认照来路回传（施工 RP-1）。

use std::collections::{BTreeMap, BTreeSet};

use crate::support::{
    call, deepseek, file, id, image, private_thought, read_tool, sample, sees_all, text, texts,
    thought, tool_call,
};
use miyu_drivers::Inputs;
use miyu_drivers::openai_chat::{
    Compat, EncodeError, ReasoningField, ReasoningReplay, blobs_needed, encode,
};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const SHOT: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rscreenshot";
const PDF: &[u8] = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n";
/// 压缩包的开头：签名、版本号，版本号的高位是 0。有 NUL，不是文本（施工 3-9 三补：原来只有 4 个字节的签名，没有 NUL，
/// 会被当成文本文件照字发）。
const ZIP: &[u8] = b"PK\x03\x04\x14\x00";

fn request(messages: Vec<Message>) -> Request {
    Request {
        tools: vec![read_tool()],
        system: "You are a helpful assistant.".to_string(),
        messages,
        stable: 0,
        continuation: false,
        described: Default::default(),
        output_cap: None,
        used: None,
    }
}

fn blobs() -> BTreeMap<ContentHash, Vec<u8>> {
    [PNG, SHOT, PDF, ZIP]
        .into_iter()
        .map(|content| (ContentHash::of(content), content.to_vec()))
        .collect()
}

fn body(request: &Request, inputs: Inputs, compat: &Compat) -> Vec<u8> {
    encode(request, &call(inputs, None), compat, &texts(), &blobs())
        .expect("要用的 blob 都在")
        .body
}

/// 一条 user 消息：字、图、字、PDF、一个压缩包。
fn attachments() -> Request {
    request(vec![Message::User {
        blocks: vec![
            text("看看这张图和这份报告"),
            image(PNG, "image/png"),
            text("还有这个"),
            file(PDF, "报告.pdf", "application/pdf"),
            file(ZIP, "data.zip", "application/zip"),
        ],
    }])
}

#[test]
fn images_and_pdfs_become_data_urls() {
    // 分成几段；压缩包发不了，换成占位，接在最后一段字里。
    sample(
        "media",
        &body(&attachments(), sees_all(), &Compat::default()),
    );
}

#[test]
fn a_model_that_cannot_see_gets_placeholders() {
    // 全是文字了，拼成一个字符串。
    sample(
        "media-omitted",
        &body(&attachments(), Inputs::default(), &Compat::default()),
    );
}

/// 两次调用：一次回了字和一张图，一次只回了一张图；然后她接着说。
fn tool_attachments() -> Request {
    request(vec![
        Message::User {
            blocks: vec![text("截两张图")],
        },
        Message::Assistant {
            blocks: vec![
                tool_call("call_2_1", "read", r#"{"path":"a.png"}"#, None),
                tool_call("call_2_2", "read", r#"{"path":"b.png"}"#, None),
            ],
        },
        Message::Tool {
            call_id: id("call_2_1"),
            error: false,
            blocks: vec![text("截好了"), image(PNG, "image/png")],
        },
        Message::Tool {
            call_id: id("call_2_2"),
            error: false,
            blocks: vec![image(SHOT, "image/png")],
        },
        Message::Assistant {
            blocks: vec![text("两张都看到了。")],
        },
    ])
}

#[test]
fn images_in_tool_results_move_after_the_results() {
    // 这串 tool 消息之后插一条 user 消息：先一句说明，再照先后放图；只回了图的那条写「在下一条」。
    sample(
        "tool-attachments",
        &body(&tool_attachments(), sees_all(), &Compat::default()),
    );
}

#[test]
fn images_in_tool_results_become_placeholders_when_the_model_cannot_see() {
    sample(
        "tool-attachments-omitted",
        &body(&tool_attachments(), Inputs::default(), &Compat::default()),
    );
}

/// 一次带思考的回答，思考和正文交错着来；一次只有工具调用、没有思考的。
fn thinking() -> Request {
    request(vec![
        Message::User {
            blocks: vec![text("算一下")],
        },
        Message::Assistant {
            blocks: vec![
                thought("先想想"),
                text("答案是"),
                thought("，再想想"),
                text(" 42。"),
            ],
        },
        Message::User {
            blocks: vec![text("再读一下文件")],
        },
        Message::Assistant {
            blocks: vec![tool_call("call_4_1", "read", r#"{"path":"a"}"#, None)],
        },
        Message::Tool {
            call_id: id("call_4_1"),
            error: false,
            blocks: vec![text("1")],
        },
    ])
}

#[test]
fn reasoning_can_be_dropped() {
    let compat = Compat {
        reasoning: ReasoningReplay::Drop,
        ..Compat::default()
    };
    sample(
        "reasoning-dropped",
        &body(&thinking(), Inputs::default(), &compat),
    );
}

/// 默认照来路回传（施工 RP-1）：几条 assistant，思考各有各的来路。
fn echoed() -> Request {
    let reply = |blocks| Message::Assistant { blocks };
    let ask = || Message::User {
        blocks: vec![text("接着")],
    };
    request(vec![
        ask(),
        // 来自 reasoning_content：各块接上，写回 reasoning_content；后面没挂私有数据的那块也接上。
        reply(vec![
            private_thought("先想想", "openai-chat", r#"{"field":"reasoning_content"}"#),
            text("答案是"),
            thought("，再想想"),
        ]),
        ask(),
        // 来自 reasoning：写回 reasoning。
        reply(vec![
            private_thought("换个字段", "openai-chat", r#"{"field":"reasoning"}"#),
            text("好"),
        ]),
        ask(),
        // 以前的日志：没记来路，不写。
        reply(vec![thought("老的"), text("嗯")]),
        ask(),
        // 别的驱动的思考（换过模型）：不写，数据里碰巧也写着 field 的照样不认。
        reply(vec![
            private_thought(
                "别家的",
                "openai-responses",
                r#"{"field":"reasoning_content"}"#,
            ),
            text("哦"),
        ]),
        ask(),
        // 来自 reasoning_text：没实测过哪家收，不写。
        reply(vec![
            private_thought("没见过的", "openai-chat", r#"{"field":"reasoning_text"}"#),
            text("行"),
        ]),
        ask(),
        // 没有思考：不写。
        reply(vec![text("直接答")]),
    ])
}

#[test]
fn reasoning_goes_back_where_it_came_from_by_default() {
    assert_eq!(Compat::default().reasoning, ReasoningReplay::Echo);
    let body = body(&echoed(), Inputs::default(), &Compat::default());
    let value: serde_json::Value = serde_json::from_slice(&body).expect("是 JSON");
    let replies: Vec<&serde_json::Value> = value["messages"]
        .as_array()
        .expect("有消息")
        .iter()
        .filter(|message| message["role"] == "assistant")
        .collect();
    let fields = |message: &serde_json::Value| {
        (
            message.get("reasoning_content").cloned(),
            message.get("reasoning").cloned(),
        )
    };
    let some = |text: &str| Some(serde_json::Value::from(text));
    assert_eq!(fields(replies[0]), (some("先想想，再想想"), None));
    assert_eq!(fields(replies[1]), (None, some("换个字段")));
    for (i, reply) in replies.iter().enumerate().skip(2) {
        assert_eq!(fields(reply), (None, None), "第 {} 条不该带思考", i + 1);
    }
    // 先后照旧：content 后面是思考。
    let text = String::from_utf8(body).expect("UTF-8");
    assert!(
        text.contains(
            r#"{"role":"assistant","content":"答案是","reasoning_content":"先想想，再想想"}"#
        ),
        "{text}"
    );
}

#[test]
fn deepseek_gets_reasoning_content_on_every_assistant() {
    // 思考、正文各自直接接上，它们本来就是一整段；没有思考的那条写空串。
    sample(
        "reasoning-deepseek",
        &body(&thinking(), Inputs::default(), &deepseek()),
    );
}

#[test]
fn reasoning_can_go_into_the_reasoning_field() {
    let compat = Compat {
        reasoning: ReasoningReplay::Replay {
            field: ReasoningField::Reasoning,
            always: false,
        },
        ..Compat::default()
    };
    sample(
        "reasoning-field",
        &body(&thinking(), Inputs::default(), &compat),
    );
}

#[test]
fn a_missing_blob_is_reported() {
    let error = encode(
        &attachments(),
        &call(sees_all(), None),
        &Compat::default(),
        &texts(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(error, EncodeError::MissingBlob(ContentHash::of(PNG)));
    assert!(error.to_string().contains(ContentHash::of(PNG).as_str()));
}

#[test]
fn the_blobs_it_needs_follow_what_the_model_can_take() {
    // 图片照能不能看图；文件每一个都要（施工 3-9 三补）：发不了的也要认是不是文本、要写有多大。
    let request = attachments();
    let files = [ContentHash::of(PDF), ContentHash::of(ZIP)];
    let all: BTreeSet<ContentHash> =
        [ContentHash::of(PNG), files[0].clone(), files[1].clone()].into();
    assert_eq!(blobs_needed(&request, &call(sees_all(), None)), all);
    let images_only = Inputs {
        images: true,
        pdf: false,
    };
    assert_eq!(blobs_needed(&request, &call(images_only, None)), all);
    assert_eq!(
        blobs_needed(&request, &call(Inputs::default(), None)),
        BTreeSet::from(files)
    );
    // 工具结果里的也算；只取要用的：编码时一个都不缺。
    let request = tool_attachments();
    let needed = blobs_needed(&request, &call(sees_all(), None));
    let taken: BTreeMap<ContentHash, Vec<u8>> = blobs()
        .into_iter()
        .filter(|(hash, _)| needed.contains(hash))
        .collect();
    assert!(
        encode(
            &request,
            &call(sees_all(), None),
            &Compat::default(),
            &texts(),
            &taken
        )
        .is_ok()
    );
}
