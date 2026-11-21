//! 一条消息带的东西（施工 FX-16 从 `entry.rs` 挪出来）：附件（内容在核心里的、只有路径的），和场所消息带的、没下载进来的平台媒体。

use serde::{Deserialize, Serialize};

use miyu_kernel::event::Media;
use miyu_kernel::id::ContentHash;

use super::is_false;

/// 一件附件。只有路径的（施工 AT-1，`internal/blueprint/view.md`「条目」的 `user`）没有 `media_type`、`blob`，带着 `path`、
/// `path_only`，头写了大小的有 `size`；别的两格都有。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// `image` 或 `file`；只有路径的是 `file`。
    pub kind: String,
    /// 名字；图可以没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 类型；只有路径的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// 内容的哈希：头照 `blob.get` 取；只有路径的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<ContentHash>,
    /// 图的宽。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// 图的高。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// 带了路径的附件的路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// 只有路径的附件，头写的字节数（施工 AT-1）；别的附件的大小头照 `blob` 取。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// 只有路径的附件（施工 AT-1）：内容没存进核心，取不到，只有名字和路径。
    #[serde(default, skip_serializing_if = "is_false")]
    pub path_only: bool,
}

/// 场所消息带的一样没下载进来的平台媒体（施工 FX-16，`internal/blueprint/view.md`「条目」的 `user`）：只记种类和平台给的名字、大小，
/// 平台的编号不交（头取不了，要看由她调工具去取）。头照 `kind` 写「\[图片\]」「\[表情\]」「\[视频\]」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueMedia {
    /// `image`、`file`、`voice`、`video`、`sticker`。
    pub kind: String,
    /// 文件的文件名、表情的字：平台给的，不可信，头照常转义；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 多少字节：平台说了的才有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

impl VenueMedia {
    /// `media` 里没当内容块交进来的那几样，照先后。
    #[must_use]
    pub(crate) fn unshown(media: &[Media]) -> Vec<VenueMedia> {
        media
            .iter()
            .filter(|media| !media.shown)
            .map(|media| VenueMedia {
                kind: media.kind.as_str().to_string(),
                name: media.name.clone(),
                size: media.size,
            })
            .collect()
    }
}
