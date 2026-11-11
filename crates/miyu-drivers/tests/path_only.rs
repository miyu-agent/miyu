//! 只有路径的附件怎么进请求（施工 AT-1，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：没有内容可发，三个驱动都写同一句，
//! 带名字、路径，头写了大小的带大小；模型能看什么都一样，不要 blob；和别的块混着先后不乱；工具结果里的照字写；以前造的
//! 快照里没有那两句的只写路径本身。

use std::collections::BTreeMap;

use crate::support::{call, claude, gpt, id, read_tool, sees_all, text, texts, tool_call};
use miyu_drivers::openai_chat::Compat;
use miyu_drivers::{
    DriverTextSources, DriverTexts, Inputs, anthropic, openai_chat, openai_responses,
};
use miyu_kernel::block::{Block, PathOnly};
use miyu_kernel::id::{ContentHash, FileName, SourcePath};
use miyu_kernel::request::{Message, Request};
use serde_json::Value;

/// 写了大小的那一句，照出厂的原文。
const SIZED: &str = "A file was attached here (disk.iso, 31457280 bytes, at /home/alice/Downloads/disk.iso). \
                     Its content is not included. Tools can read it.\n";
/// 没写大小的那一句。
const BARE: &str = "A file was attached here (笔记.md, at ~/笔记.md). Its content is not included. \
                    Tools can read it.\n";

fn path_only(name: &str, path: &str, size: Option<u64>) -> Block {
    Block::Path(PathOnly {
        name: FileName::parse(name).expect("文件名合写法"),
        path: SourcePath::parse(path).expect("路径合写法"),
        size,
    })
}

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
        tail: None,
    }
}

/// 一句话、一个写了大小的、一个没写的、又一句话。
fn attached() -> Request {
    request(vec![Message::User {
        blocks: vec![
            text("看看这两个"),
            path_only(
                "disk.iso",
                "/home/alice/Downloads/disk.iso",
                Some(31_457_280),
            ),
            path_only("笔记.md", "~/笔记.md", None),
            text("先说哪个大。"),
        ],
    }])
}

/// 一个 blob 都不交。
fn none() -> BTreeMap<ContentHash, Vec<u8>> {
    BTreeMap::new()
}

fn json(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("是 JSON")
}

#[test]
fn every_driver_writes_the_same_sentence_whatever_the_model_can_take() {
    let expected = format!("看看这两个\n{SIZED}{BARE}先说哪个大。");
    for inputs in [sees_all(), Inputs::default()] {
        let chat = openai_chat::encode(
            &attached(),
            &call(inputs, None),
            &Compat::default(),
            &texts(),
            &none(),
        )
        .expect("不要 blob")
        .body;
        assert_eq!(json(&chat)["messages"][1]["content"], expected.as_str());
        let responses =
            openai_responses::encode(&attached(), &gpt(inputs, None), &texts(), &none())
                .expect("不要 blob")
                .body;
        assert_eq!(json(&responses)["input"][0]["content"], expected.as_str());
        let claude = anthropic::encode(&attached(), &claude(inputs, None), &texts(), &none())
            .expect("不要 blob")
            .body;
        let text = String::from_utf8(claude).expect("UTF-8");
        let said: Vec<String> = json(text.as_bytes())["messages"][0]["content"]
            .as_array()
            .expect("几块")
            .iter()
            .map(|part| part["text"].as_str().expect("都是字").to_string())
            .collect();
        assert_eq!(said.concat().replace('\n', ""), expected.replace('\n', ""));
        assert!(said.contains(&SIZED.to_string()), "{said:?}");
        assert!(said.contains(&BARE.to_string()), "{said:?}");
        // 这种附件一个 blob 都不要。
        assert!(openai_chat::blobs_needed(&attached(), &call(inputs, None)).is_empty());
        assert!(anthropic::blobs_needed(&attached(), &claude_call(inputs)).is_empty());
        assert!(openai_responses::blobs_needed(&attached(), &gpt(inputs, None)).is_empty());
    }
}

fn claude_call(inputs: Inputs) -> miyu_drivers::Call {
    claude(inputs, None)
}

/// 名字、路径照模板的规矩转义：伪造不了标签。
#[test]
fn the_name_and_the_path_are_escaped() {
    let request = request(vec![Message::User {
        blocks: vec![path_only("<x>.iso", "/data/\"><y>.iso", Some(0))],
    }]);
    let body = openai_chat::encode(
        &request,
        &call(Inputs::default(), None),
        &Compat::default(),
        &texts(),
        &none(),
    )
    .expect("不要 blob")
    .body;
    let said = json(&body)["messages"][1]["content"]
        .as_str()
        .expect("一段字")
        .to_string();
    assert!(!said.contains('<') && !said.contains('"'), "{said}");
    assert!(said.contains("0 bytes"), "大小是 0 的也写：{said}");
}

/// 工具结果里的（现在没有谁会造，写法一样）照字写进结果；assistant 消息里的不写。
#[test]
fn in_a_tool_result_it_is_written_in_place() {
    let request = request(vec![
        Message::User {
            blocks: vec![text("找一下")],
        },
        Message::Assistant {
            blocks: vec![
                path_only("a.bin", "/a.bin", None),
                tool_call("call_2_1", "read", r#"{"path":"a"}"#, None),
            ],
        },
        Message::Tool {
            call_id: id("call_2_1"),
            error: false,
            blocks: vec![
                text("在这"),
                path_only("disk.iso", "/data/disk.iso", Some(7)),
            ],
        },
    ]);
    let body = openai_chat::encode(
        &request,
        &call(sees_all(), None),
        &Compat::default(),
        &texts(),
        &none(),
    )
    .expect("不要 blob")
    .body;
    let body = json(&body);
    assert_eq!(
        body["messages"][3]["content"],
        "在这\nA file was attached here (disk.iso, 7 bytes, at /data/disk.iso). \
         Its content is not included. Tools can read it.\n"
    );
    assert!(
        !body["messages"][2].to_string().contains("a.bin"),
        "assistant 消息里的不写：{body}"
    );
}

/// 以前造的快照里没有那两句：只写路径本身，照模板的规矩转义。
#[test]
fn an_older_snapshot_writes_only_the_path() {
    let old = DriverTexts::new(DriverTextSources {
        image_omitted: include_str!("../../../resources/core/drivers/image-omitted.txt"),
        file_omitted: include_str!("../../../resources/core/drivers/file-omitted.txt"),
        no_output: include_str!("../../../resources/core/drivers/no-output.txt"),
        tool_attachments: include_str!("../../../resources/core/drivers/tool-attachments.txt"),
        tool_attachments_only: include_str!(
            "../../../resources/core/drivers/tool-attachments-only.txt"
        ),
        text_file: None,
        image_name: None,
        image_description: None,
        attached_path: None,
        open_path: None,
        path_only: None,
    })
    .expect("用得了");
    let body = openai_chat::encode(
        &attached(),
        &call(Inputs::default(), None),
        &Compat::default(),
        &old,
        &none(),
    )
    .expect("不要 blob")
    .body;
    assert_eq!(
        json(&body)["messages"][1]["content"],
        "看看这两个\n/home/alice/Downloads/disk.iso\n~/笔记.md\n先说哪个大。"
    );
}
