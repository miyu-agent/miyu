//! 金样用的这一批请求（施工 V-38，从 `golden.rs` 拆出来）：几样附件的字节、请求、驱动的占位。内容都是造的，写法照真的。

use std::collections::BTreeMap;

use miyu_drivers::{
    AttachedPathSources, DriverTextSources, DriverTexts, ImageDescriptionSources, ImageNameSources,
    OpenPathSources, TextFileSources,
};
use miyu_kernel::block::{Block, File, Image, Private, Reasoning, Text, ToolCall};
use miyu_kernel::id::{
    CallId, ContentHash, DriverFamily, FileName, MediaType, ModelName, SourcePath,
};
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request, ToolSpec};

/// 几样附件的字节：内容是造的，写法照真的（PNG、JPEG、PDF 的头，大的那张图编出来过 128 KiB，走单独映射的大块）。
pub(super) struct Media {
    pub(super) png: Vec<u8>,
    pub(super) jpeg: Vec<u8>,
    pub(super) pdf: Vec<u8>,
    pub(super) notes: Vec<u8>,
}

pub(super) fn media() -> Media {
    let filler = |seed: u32, len: usize| -> Vec<u8> {
        (0..len)
            .map(|i| {
                let x = (i as u32).wrapping_mul(2_654_435_761).wrapping_add(seed);
                (x >> 13) as u8
            })
            .collect()
    };
    let png = [b"\x89PNG\r\n\x1a\n".as_slice(), &filler(1, 300_000)].concat();
    let jpeg = [b"\xff\xd8\xff\xe0".as_slice(), &filler(2, 40_000)].concat();
    let pdf = [b"%PDF-1.4\n".as_slice(), &filler(3, 20_000)].concat();
    let notes =
        "第一行：\"引号\" 和 \\反斜杠\\\r\n制表\t控制\u{1}\u{1f}\n</file> 不是结尾\n表情 🐱👍\n"
            .repeat(50)
            .into_bytes();
    Media {
        png,
        jpeg,
        pdf,
        notes,
    }
}

fn private(family: &str, data: &str) -> Private {
    Private {
        driver: DriverFamily::parse(family).expect("驱动家族合写法"),
        data: raw(data),
    }
}

fn raw(json: &str) -> RawJson {
    serde_json::from_str(json).expect("是 JSON")
}

fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

fn call_id(id: &str) -> CallId {
    CallId::parse(id).expect("调用编号合写法")
}

fn image(bytes: &[u8], name: Option<&str>, path: Option<&str>, media_type: &str) -> Block {
    Block::Image(Image {
        blob: ContentHash::of(bytes),
        name: name.map(|name| FileName::parse(name).expect("文件名合写法")),
        media_type: MediaType::parse(media_type).expect("媒体类型合写法"),
        width: 2560,
        height: 1440,
        path: path.map(|path| SourcePath::parse(path).expect("路径合写法")),
    })
}

fn file(bytes: &[u8], name: &str, media_type: &str) -> Block {
    Block::File(File {
        blob: ContentHash::of(bytes),
        name: FileName::parse(name).expect("文件名合写法"),
        media_type: MediaType::parse(media_type).expect("媒体类型合写法"),
        path: None,
    })
}

/// 这一批的请求，思考和调用的私有数据照驱动家族 `family`（`thought`、`called` 是各家自己的写法）。
pub(super) fn request(
    media: &Media,
    family: &str,
    thought: &str,
    called: &str,
    continuation: bool,
) -> Request {
    let tool = |name: &str, description: &str| ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters: raw(
            r#"{"type":"object","properties":{"path":{"type":"string","description":"绝对路径 \"quoted\""}},"required":["path"]}"#,
        ),
    };
    let user = vec![
        text("先看这张图。\n\"引号\"、反斜杠\\、制表\t、控制\u{7}、表情 🐱"),
        image(
            &media.png,
            Some("晚霞.png"),
            Some("/home/alice/图/晚霞.png"),
            "image/png",
        ),
        file(&media.pdf, "报告.pdf", "application/pdf"),
        file(&media.notes, "笔记.txt", "text/plain"),
    ];
    let reply = vec![
        Block::Reasoning(Reasoning {
            text: "先读文件，再看图。\n第二行".to_string(),
            private: Some(private(family, thought)),
        }),
        text("我看一下。"),
        Block::ToolCall(ToolCall {
            call_id: call_id("call_1_1"),
            name: "read".to_string(),
            args: r#"{"path":"/home/alice/a.txt"}"#.to_string(),
            private: Some(private(family, called)),
        }),
        Block::ToolCall(ToolCall {
            call_id: call_id("call_1_2"),
            name: "grep".to_string(),
            args: r#"{"pattern":"\\d+\"x\""}"#.to_string(),
            private: None,
        }),
    ];
    let mut described = BTreeMap::new();
    described.insert(
        ContentHash::of(&media.jpeg),
        "一张截图：终端里跑着 \"cargo test\"。".to_string(),
    );
    Request {
        tools: vec![
            tool("grep", "Search file contents."),
            tool("read", "Read a file."),
        ],
        system: "你是助手。\n规则：\"照实说\"\t\\ 不编。".to_string(),
        messages: vec![
            Message::User { blocks: user },
            Message::Assistant { blocks: reply },
            Message::Tool {
                call_id: call_id("call_1_1"),
                error: false,
                blocks: vec![
                    text("第一行\n第二行"),
                    image(&media.jpeg, None, None, "image/jpeg"),
                    text("图后面的字"),
                ],
            },
            Message::Tool {
                call_id: call_id("call_1_2"),
                error: true,
                blocks: Vec::new(),
            },
            Message::Assistant {
                blocks: vec![text("文件里写着，第一")],
            },
            Message::User {
                blocks: vec![text("从断的地方接着说。")],
            },
        ],
        stable: 1,
        continuation,
        described,
        output_cap: None,
        used: None,
    }
}

/// 驱动的几句占位：写死在这里，资源目录里的字改了金样不跟着变。
pub(super) fn texts() -> DriverTexts {
    DriverTexts::new(DriverTextSources {
        image_omitted: "An image was attached here, but this model cannot view images.",
        file_omitted: "A file was attached here ({name}, {media_type}, {size} bytes), but this model cannot read it.",
        no_output: "(no output)",
        tool_attachments: "Images and files returned by the tool calls above.",
        tool_attachments_only: "The tool returned only images or files, in the next message.",
        text_file: Some(TextFileSources {
            file_open: "<file name=\"{name}\">",
            file_cut: "Only the first {shown} of {total} bytes are shown.",
            file_close: "</file>",
        }),
        image_name: Some(ImageNameSources {
            image_open: "<image name=\"{name}\">",
            image_close: "</image>",
            image_omitted_named: "An image was attached here ({name}), but this model cannot view images.",
        }),
        image_description: Some(ImageDescriptionSources {
            image_description_open: "<image-description>",
            image_description_open_named: "<image-description name=\"{name}\">",
            image_description_close: "</image-description>",
        }),
        attached_path: Some(AttachedPathSources {
            image_omitted_path: "An image was attached here ({name}, from {path}), but this model cannot view images.",
            file_omitted_path: "A file was attached here ({name}, {media_type}, {size} bytes, from {path}), but this model cannot read it.",
        }),
        open_path: Some(OpenPathSources {
            image_open_path: "<image name=\"{name}\" path=\"{path}\">",
            file_open_path: "<file name=\"{name}\" path=\"{path}\">",
        }),
    })
    .expect("占位用得了")
}

pub(super) fn model(name: &str) -> ModelName {
    ModelName::parse(name).expect("模型名合写法")
}
