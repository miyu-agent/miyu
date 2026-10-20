use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_mcp::{Called, Content};

use super::*;

/// 1×1 的 PNG。
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

fn texts() -> Texts {
    Texts {
        unavailable: "U {name}\n".to_string(),
        timed_out: "T {name} {seconds}\n".to_string(),
        crashed: "C {name}\n".to_string(),
        not_shown: "[{mime}, {bytes} bytes, not shown]\n".to_string(),
        cut: "[{cut} more characters were cut]\n".to_string(),
    }
}

fn called(content: Vec<Content>) -> Called {
    Called {
        content,
        structured: None,
        is_error: false,
    }
}

fn words(done: &Done) -> Vec<&str> {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(Text { text }) => text.as_str(),
            other => panic!("不是字：{other:?}"),
        })
        .collect()
}

/// 文字照原文、照先后；`isError` 的算出错。
#[test]
fn text_is_kept_and_an_error_stays_an_error() {
    let done = done(
        Called {
            is_error: true,
            ..called(vec![
                Content::Text("one".to_string()),
                Content::Text("two".to_string()),
            ])
        },
        &texts(),
    );
    assert!(done.error);
    assert_eq!(words(&done), ["one", "two"]);
}

/// 收得了的图交回成图（执行器存）；解不开、不是那四种的写一行说没给她，照 base64 的长度算字节。
#[test]
fn a_picture_is_handed_over_and_anything_else_is_named() {
    let done = done(
        called(vec![
            Content::Image {
                data: PNG.to_string(),
                mime: "image/png".to_string(),
            },
            Content::Text("a picture".to_string()),
            Content::Image {
                data: "aGk=".to_string(),
                mime: "image/png".to_string(),
            },
            Content::Audio {
                data: "aGVsbG8=".to_string(),
                mime: "audio/wav".to_string(),
            },
        ]),
        &texts(),
    );
    assert_eq!(done.images.len(), 1);
    assert_eq!((done.images[0].width, done.images[0].height), (1, 1));
    assert_eq!(
        words(&done),
        [
            "a picture",
            "[image/png, 2 bytes, not shown]",
            "[audio/wav, 5 bytes, not shown]"
        ]
    );
}

/// 链接写成 `[名字](地址)`，没名字的写地址；带文字的资源照文字，二进制的写一行，都没有的写地址；认不出的照紧凑的 JSON。
#[test]
fn links_resources_and_unknown_content_become_lines() {
    let done = done(
        called(vec![
            Content::Link {
                uri: "file:///notes.txt".to_string(),
                name: Some("notes".to_string()),
            },
            Content::Link {
                uri: "https://example.invalid/".to_string(),
                name: None,
            },
            Content::Resource {
                uri: "file:///a.txt".to_string(),
                mime: Some("text/plain".to_string()),
                text: Some("inside".to_string()),
                blob: None,
            },
            Content::Resource {
                uri: "file:///a.bin".to_string(),
                mime: None,
                text: None,
                blob: Some("AAAA".to_string()),
            },
            Content::Resource {
                uri: "file:///empty".to_string(),
                mime: None,
                text: None,
                blob: None,
            },
            Content::Other(json!({"type": "hologram", "depth": 3})),
        ]),
        &texts(),
    );
    assert_eq!(
        words(&done),
        [
            "[notes](file:///notes.txt)",
            "https://example.invalid/",
            "inside",
            "[application/octet-stream, 3 bytes, not shown]",
            "file:///empty",
            r#"{"depth":3,"type":"hologram"}"#,
        ]
    );
}

/// 一段文字都没有的照结构化结果写紧凑的 JSON；有文字的不另写；什么都没有的交一段空的。结构化结果另外原样交给脚本。
#[test]
fn structured_content_stands_in_only_when_there_is_no_text() {
    let structured = done(
        Called {
            structured: Some(json!({"answer": 42})),
            ..called(Vec::new())
        },
        &texts(),
    );
    assert_eq!(words(&structured), [r#"{"answer":42}"#]);
    let both = done(
        Called {
            structured: Some(json!({"answer": 42})),
            ..called(vec![Content::Text("42".to_string())])
        },
        &texts(),
    );
    assert_eq!(words(&both), ["42"]);
    assert_eq!(words(&done(called(Vec::new()), &texts())), [""]);
    // 给脚本的（施工 M-2）：有没有文字都原样交；没有的不交，出错的不交。
    assert_eq!(structured.data, Some(json!({"answer": 42})));
    assert_eq!(both.data, Some(json!({"answer": 42})));
    assert_eq!(done(called(Vec::new()), &texts()).data, None);
    let failed = done(
        Called {
            structured: Some(json!({"answer": 42})),
            is_error: true,
            ..called(vec![Content::Text("boom".to_string())])
        },
        &texts(),
    );
    assert_eq!(failed.data, None);
}

/// 文字加起来超过 10 万个字符的，留前 10 万个（截在中间的那一段截短，后面的不要），末尾写一行截掉了多少。
#[test]
fn long_text_is_cut_at_the_limit() {
    let one = done(called(vec![Content::Text("x".repeat(LIMIT + 5))]), &texts());
    let words_of_one = words(&one);
    assert_eq!(words_of_one.len(), 2);
    assert_eq!(words_of_one[0].chars().count(), LIMIT);
    assert_eq!(words_of_one[1], "[5 more characters were cut]");
    let many = done(
        called(vec![
            Content::Text("a".repeat(60_000)),
            Content::Text("字".repeat(60_000)),
            Content::Text("c".repeat(10)),
        ]),
        &texts(),
    );
    let words_of_many = words(&many);
    assert_eq!(words_of_many.len(), 3);
    assert_eq!(words_of_many[1], "字".repeat(40_000));
    assert_eq!(words_of_many[2], "[20010 more characters were cut]");
    let exact = done(called(vec![Content::Text("y".repeat(LIMIT))]), &texts());
    assert_eq!(words(&exact).len(), 1, "正好在线上的不截");
}
