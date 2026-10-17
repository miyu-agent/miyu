//! `show`（施工 D-5，`docs/blueprint/tools/show.md`）：读、认种类、报 `file.shown` 的每一格，她只收到一句；标题去空白；
//! 太大、目录、没有的照共用的几句；链接照真实的位置。

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::event::Said;
use miyu_kernel::tool::Access;
use miyu_tool::{Done, Effect, ShownFile};

use crate::support::{Site, tool};

/// 交回的字。
fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 报的那一条 `file.shown`。
fn shown(done: &Done) -> &ShownFile {
    match done.effects.as_slice() {
        [Effect::Shown(shown)] => shown,
        other => panic!("该报一条 file.shown：{other:?}"),
    }
}

/// 一张 1×2 的 PNG 的头。
fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&1u32.to_be_bytes());
    bytes.extend_from_slice(&2u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

#[test]
fn show_reads_and_names_what_it_reads() {
    let tool = tool("show");
    assert_eq!(tool.spec().access, Access::Read);
    assert!(
        tool.spec()
            .description
            .starts_with("Show a file to the user")
    );
}

#[tokio::test]
async fn a_picture_is_shown_with_every_field_and_she_gets_one_line() {
    let site = Site::new();
    site.file("work/pic.png", &png());
    let done = site
        .done("show", json!({"path": "pic.png", "title": "  A chart \n"}))
        .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "Shown to the user.\n", "内容不回给她");
    assert_eq!(
        done.human,
        Some(Said::new("software/basesystem/show/shown"))
    );
    assert_eq!(
        shown(&done),
        &ShownFile {
            path: site.real("work/pic.png"),
            content: png(),
            name: "pic.png".to_string(),
            media_type: "image/png".to_string(),
            title: Some("A chart".to_string()),
            language: None,
            size: Some((1, 2)),
        }
    );
}

#[tokio::test]
async fn code_and_markdown_are_named_by_their_extension() {
    let site = Site::new();
    site.file("work/main.rs", b"fn main() {}\n");
    site.file("work/notes.md", b"# Notes\n");
    let code = site
        .done("show", json!({"file_path": "main.rs", "title": "   "}))
        .await;
    let code = shown(&code);
    assert_eq!(
        (
            code.media_type.as_str(),
            code.language.as_deref(),
            code.title.as_deref()
        ),
        ("text/plain", Some("rust"), None),
        "写成 file_path 的也认，空的标题当没写"
    );
    let notes = site.done("show", json!({"path": "notes.md"})).await;
    assert_eq!(shown(&notes).media_type, "text/markdown");
}

#[tokio::test]
async fn too_big_a_directory_or_nothing_is_not_shown() {
    let site = Site::new();
    site.file("work/big.bin", &vec![0u8; 20 * 1024 * 1024 + 1]);
    site.file("work/dir/a.txt", b"a");
    let big = site.done("show", json!({"path": "big.bin"})).await;
    assert!(big.error);
    assert_eq!(
        text(&big),
        "big.bin is larger than 20 MiB and cannot be shown.\n"
    );
    assert!(big.effects.is_empty(), "什么都不报");
    for path in ["dir", "missing.png"] {
        let done = site.done("show", json!({"path": path})).await;
        assert!(done.error, "{path}");
        assert!(done.effects.is_empty(), "{path}");
    }
    let exact = site.done("show", json!({"path": "dir/a.txt"})).await;
    assert!(!exact.error, "{}", text(&exact));
}

/// 链接照 `read` 的规矩换成真实的位置（权限也照它判）：展示的是它指向的那个文件，路径、名字都是那个文件的。
#[cfg(unix)]
#[tokio::test]
async fn a_link_shows_the_file_it_points_to() {
    let site = Site::new();
    site.file("work/real.txt", b"x");
    std::os::unix::fs::symlink(site.real("work/real.txt"), site.0.join("work/link.txt"))
        .expect("建得了链接");
    let done = site.done("show", json!({"path": "link.txt"})).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(shown(&done).path, site.real("work/real.txt"));
    assert_eq!(shown(&done).name, "real.txt");
}
