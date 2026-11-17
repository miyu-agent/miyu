//! 她发出去的东西（施工 X-19 图，施工 SD-1 语音）：工具交的 `tool.result.sent`，不发给模型；头画在正文里，通讯平台照它发。
//!
//! 一项是一张图或者一段语音，JSON 里不加种类的格：写了 `"voice": true` 的是语音，别的照图读（以前的日志只有图，读进来再写
//! 出去一字不差）。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::block::Image;
use crate::id::{ContentHash, FileName, MediaType};

/// 她发出去的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Sent {
    /// 一段语音（施工 SD-1）：先试它，没写 `"voice": true` 的不是。
    Voice(Voice),
    /// 一张图（施工 X-19）。
    Image(SentImage),
}

/// 她发出去的一张图（施工 X-19）：图本身同图片块；`alt` 给看不了图的地方的一句（表情包的描述），`sticker` 是表情包（通讯平台
/// 照表情发）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentImage {
    /// 图：blob、媒体类型、宽高，同图片块。
    #[serde(flatten)]
    pub image: Image,
    /// 看不了图时的一句。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// 是表情包。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sticker: bool,
}

/// 她说出来的一段语音（施工 SD-1，`say` 默认合成以后交的）：头在正文里画播放条，照这一轮是谁说的话决定放不放，通讯平台发成
/// 语音消息；本机会话里没人在场的那一轮、没有头开着这个会话的，核心交给这台电脑的音箱放（`miyu-endpoint` 的 `provide/voice.rs`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Voice {
    /// 音频存成的 blob。
    pub blob: ContentHash,
    /// 文件名，例如 `say-1.mp3`；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<FileName>,
    /// `audio/` 开头的媒体类型，例如 `audio/mpeg`、`audio/wav`。
    pub media_type: MediaType,
    /// 是语音：JSON 里写 `true`，认它分开图和语音。
    pub voice: VoiceMark,
    /// 多长，毫秒；工具没报的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// `"voice": true` 这一格：只认 `true`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VoiceMark;

impl Serialize for VoiceMark {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for VoiceMark {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<VoiceMark, D::Error> {
        match bool::deserialize(deserializer)? {
            true => Ok(VoiceMark),
            false => Err(serde::de::Error::custom("voice must be true")),
        }
    }
}

impl Sent {
    /// 这一项的 blob。
    #[must_use]
    pub fn blob(&self) -> &ContentHash {
        match self {
            Sent::Voice(voice) => &voice.blob,
            Sent::Image(sent) => &sent.image.blob,
        }
    }
}
