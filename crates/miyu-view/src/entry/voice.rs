//! 她说出来的一段语音（施工 SD-1，`docs/blueprint/view.md`「条目」的 `voice`）：工具交的 `tool.result.sent` 里写了 `voice` 的那一项，
//! 画在正文里一个播放条。放不放由头照这一轮是谁开的定：`cause` 是自己发的那条请求的编号的，是这个头上说的话，当场放；`by`
//! 是到点叫醒的（`schedule`）、没有的，是没人开口的一轮，开着这个会话的头都放；别的头上说的只画播放条。

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use miyu_kernel::id::{CommandId, ContentHash};
use miyu_kernel::origin::By;

/// 一段语音。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Voice {
    /// 音频：头照 `blob.get` 取。
    pub blob: ContentHash,
    /// `audio/` 开头的媒体类型。
    pub media_type: String,
    /// 文件名；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 多长，毫秒；工具没报的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 开这一轮的那句话是谁说的，同事件的 `by`（本机的头上说的带 `head`）；这一轮没有触发的那句的不写。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "by"
    )]
    pub by: Option<By>,
    /// 开这一轮的那条请求的编号，同 `turn.started` 的 `cause`：头照它认出是不是自己发的话。没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<CommandId>,
}

/// 读 `by`：先读成 JSON 再照 `By` 读（同 [`super::Sender`]）。`by` 自己的读法认不了条目照种类分开时缓下来的那一份。
fn by<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<By>, D::Error> {
    let value = Option::<Value>::deserialize(deserializer)?;
    value
        .map(serde_json::from_value)
        .transpose()
        .map_err(serde::de::Error::custom)
}
