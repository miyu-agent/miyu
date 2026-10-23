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

/// 音视频（施工 D-5 补）：照文件头认七种，QuickTime、Matroska、AAC 的 ADTS 不认；认不出的二进制照后缀；文本照旧是文本。
#[test]
fn audio_and_video_are_told_by_their_heads_then_their_extensions() {
    let mp4 = b"\0\0\0\x20ftypisom\0\0\x02\0isomiso2";
    assert_eq!(of("a.bin", mp4).media_type, "video/mp4");
    assert_eq!(
        of("a.bin", b"\0\0\0\x20ftypM4A \0\0\0\0").media_type,
        "audio/mp4"
    );
    assert_eq!(
        of("a.mov", b"\0\0\0\x14ftypqt  \0\0\0\0\xFF").media_type,
        "application/octet-stream"
    );
    let mut webm = vec![
        0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 0x01, 0x42, 0x82, 0x84,
    ];
    webm.extend_from_slice(b"webm\x42\x87\x81\x04");
    assert_eq!(of("a.bin", &webm).media_type, "video/webm");
    let mut mkv = vec![0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x82, 0x88];
    mkv.extend_from_slice(b"matroska");
    assert_eq!(of("a.mkv", &mkv).media_type, "application/octet-stream");
    assert_eq!(of("a.bin", b"ID3\x04\0\0\0\0\0\0").media_type, "audio/mpeg");
    assert_eq!(
        of("a.bin", &[0xFF, 0xFB, 0x90, 0x64, 0x00]).media_type,
        "audio/mpeg"
    );
    assert_eq!(
        of("a.aac", &[0xFF, 0xF1, 0x50, 0x80, 0x00]).media_type,
        "application/octet-stream",
        "ADTS 的层是 00，不是 MP3"
    );
    assert_eq!(of("a.bin", b"fLaC\0\0\0\x22").media_type, "audio/flac");
    assert_eq!(
        of("a.bin", b"RIFF\x24\0\0\0WAVEfmt ").media_type,
        "audio/wav"
    );
    assert_eq!(of("a.bin", b"OggS\0\x02\0\0").media_type, "audio/ogg");
    assert_eq!(
        of("a.avi", b"RIFF\x24\0\0\0AVI LIST\xFF").media_type,
        "application/octet-stream",
        "RIFF 里不是 WAVE 的不认"
    );
    // 文件头认不出的二进制照后缀。
    let odd = [0x00, 0xFE, 0x81, 0x99];
    for (name, media_type) in [
        ("a.mp4", "video/mp4"),
        ("a.webm", "video/webm"),
        ("a.MP3", "audio/mpeg"),
        ("a.flac", "audio/flac"),
        ("a.wav", "audio/wav"),
        ("a.m4a", "audio/mp4"),
        ("a.opus", "audio/ogg"),
        ("a.mkv", "application/octet-stream"),
    ] {
        assert_eq!(of(name, &odd).media_type, media_type, "{name}");
    }
    assert_eq!(
        of("notes.mp3", b"just text").media_type,
        "text/plain",
        "文本照旧"
    );
}
