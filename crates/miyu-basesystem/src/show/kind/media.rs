//! 音视频（施工 D-5 补，网页报、项目主人 2026-10-23 指出：展示的 mp4、mp3 只能下载）：浏览器放得了的七种，先照内容的文件头认，
//! 认不出的二进制再照后缀。mov、mkv 浏览器多半放不了，照别的文件。

/// 认文件头看前面多少字节：WebM 的文档类型在 EBML 头里，一般在前 40 字节以内。
pub(super) const HEAD: usize = 64;

/// 照文件头认：MP4 的 `ftyp`（`M4A `、`M4B ` 牌子的是音频，QuickTime 的 `qt  ` 不认）、WebM（EBML 头、文档类型写 `webm`；
/// Matroska 不认）、MP3（`ID3` 或者第三层的帧同步）、FLAC、WAV、Ogg。
pub(super) fn by_content(head: &[u8]) -> Option<&'static str> {
    if head.get(4..8) == Some(b"ftyp") {
        return match head.get(8..12)? {
            b"M4A " | b"M4B " => Some("audio/mp4"),
            b"qt  " => None,
            _ => Some("video/mp4"),
        };
    }
    if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        let doc = head.windows(4).any(|w| w == b"webm");
        return doc.then_some("video/webm");
    }
    if head.starts_with(b"ID3") || mp3_frame(head) {
        return Some("audio/mpeg");
    }
    if head.starts_with(b"fLaC") {
        return Some("audio/flac");
    }
    if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WAVE") {
        return Some("audio/wav");
    }
    if head.starts_with(b"OggS") {
        return Some("audio/ogg");
    }
    None
}

/// MPEG 音频第三层的帧头：11 位同步、层的两位是 `01`（AAC 的 ADTS 是 `00`，不算）。
fn mp3_frame(head: &[u8]) -> bool {
    matches!(head, [0xFF, second, ..] if second & 0xE0 == 0xE0 && (second >> 1) & 0b11 == 0b01)
}

/// 照后缀（小写）：内容认不出的二进制才看。
pub(super) fn by_extension(extension: Option<&str>) -> Option<&'static str> {
    Some(match extension? {
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "ogg" | "oga" | "opus" => "audio/ogg",
        _ => return None,
    })
}
