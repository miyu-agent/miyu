//! `show`（施工 D-5，`docs/blueprint/tools/show.md`）：认种类、报 `file.shown` 的每一格，她只收到一句；标题去空白；过了 20 MiB
//! 照常展示（施工 SW-1）；目录、没有的照共用的几句；链接照真实的位置。

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::event::Said;
use miyu_kernel::tool::Access;
use miyu_tool::{Done, Effect, ShownFile, modified_at};

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
    let metadata = std::fs::metadata(site.real("work/pic.png")).expect("在");
    assert_eq!(
        shown(&done),
        &ShownFile {
            path: site.real("work/pic.png"),
            name: "pic.png".to_string(),
            media_type: "image/png".to_string(),
            size: png().len() as u64,
            modified: modified_at(&metadata).expect("有修改时间"),
            title: Some("A chart".to_string()),
            language: None,
            pixels: Some((1, 2)),
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

/// 不限大小（施工 SW-1）：过了以前的 20 MiB 也照常展示，记的是整份的大小、修改时间，不带内容。稀疏文件，不真占盘。
#[tokio::test]
async fn a_file_over_twenty_mib_is_shown() {
    let site = Site::new();
    site.file("work/big.mp4", b"\0\0\0\x18ftypisom");
    let big = 20 * 1024 * 1024 + 1;
    std::fs::File::options()
        .write(true)
        .open(site.real("work/big.mp4"))
        .and_then(|file| file.set_len(big))
        .expect("造得出稀疏文件");
    let done = site.done("show", json!({"path": "big.mp4"})).await;
    assert!(!done.error, "{}", text(&done));
    let metadata = std::fs::metadata(site.real("work/big.mp4")).expect("在");
    let shown = shown(&done);
    assert_eq!(
        (shown.size, Some(shown.modified), shown.path.clone()),
        (big, modified_at(&metadata), site.real("work/big.mp4"))
    );
    assert_eq!(shown.media_type, "video/mp4", "照开头的文件头认");
}

#[tokio::test]
async fn a_directory_or_nothing_is_not_shown() {
    let site = Site::new();
    site.file("work/dir/a.txt", b"a");
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
