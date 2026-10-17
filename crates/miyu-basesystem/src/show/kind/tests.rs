//! `show` 认种类（施工 D-5）：照先后认，内容先于后缀；语言表；svg、空文件、非 UTF-8。

use super::*;

fn of(name: &str, bytes: &[u8]) -> Kind {
    kind(Path::new("/w").join(name).as_path(), bytes)
}

/// 一张 1×2 的 PNG 的头（量宽高只看头）。
fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&1u32.to_be_bytes());
    bytes.extend_from_slice(&2u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

#[test]
fn content_comes_before_the_extension() {
    assert_eq!(
        of("a.md", &png()),
        Kind {
            media_type: "image/png",
            language: None,
            size: Some((1, 2)),
        },
        "后缀写错了照内容"
    );
    assert_eq!(of("a.txt", b"%PDF-1.7\n").media_type, "application/pdf");
    assert_eq!(of("a.svg", b"<svg/>").media_type, "image/svg+xml");
    assert_eq!(
        of("a.HTML", b"<p>").media_type,
        "text/html",
        "后缀不分大小写"
    );
    assert_eq!(of("a.htm", b"<p>").media_type, "text/html");
    assert_eq!(of("a.markdown", b"# x").media_type, "text/markdown");
}

#[test]
fn text_carries_its_language_and_other_bytes_are_a_file() {
    assert_eq!(
        of("main.rs", b"fn main() {}"),
        Kind {
            media_type: "text/plain",
            language: Some("rust"),
            size: None,
        }
    );
    assert_eq!(of("x.yml", b"a: 1").language, Some("yaml"));
    assert_eq!(of("Dockerfile", b"FROM x").language, Some("dockerfile"));
    assert_eq!(of("notes", b"plain").language, None);
    assert_eq!(
        of("empty.txt", b"").media_type,
        "text/plain",
        "空文件是文本"
    );
    assert_eq!(
        of("data.bin", &[0xff, 0xfe, 0x00]).media_type,
        "application/octet-stream"
    );
    assert_eq!(
        of("bad.rs", &[0xc3, 0x28]).language,
        None,
        "不是 UTF-8 的不带语言"
    );
}
