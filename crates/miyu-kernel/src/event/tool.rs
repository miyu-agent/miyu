//! 工具的事件（`docs/designs/03-事件模型.md` 第三节「消息和工具结果怎么写」「确认的事件
//! 怎么写」）：调用的结果，请人确认，人的决定。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::block::{Block, Image};
use crate::event::Effect;
use crate::id::CallId;
use crate::raw::RawJson;
use crate::text_enum::text_enum;
use crate::tool::Access;

/// 她发出去的一张图（施工 X-19）：图本身同图片块；`alt` 给看不了图的地方的一句（表情包的描述），`sticker` 是表情包（通讯平台
/// 照表情发）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
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

/// `tool.result`：一个工具调用的结果。结果按到达的先后记进日志，
/// 投影时按调用的先后排（03 第六节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    /// 这是哪一次调用的结果。那次调用在哪条消息里、是第几个，编号本身就写着。
    pub call_id: CallId,
    /// 结果怎样。
    pub status: ToolStatus,
    /// 给模型看的内容。已取消、被拒绝、已跳过的，是内核写的一句英文短句。
    pub blocks: Vec<Block>,
    /// 执行用了多少毫秒：从开始执行算到结束，由执行器量好，随结果送进来。
    /// 等人确认在执行之前，不算在里面。没真执行过的（例如被拒绝、已跳过）没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 给人看的说法（施工 4-5 上）：不发给模型，头照自己的语言换成字（`26-提示词.md` 第三节「双槽」）。
    /// 工具没交、老日志里没有的，没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human: Option<Said>,
    /// 给内核和头看的效果（施工 4-6 上）：读了、改了、删了哪个文件，不发给模型。工具交的，照交的先后；
    /// 没有的（内核自己写的结果、老日志）没有这一格。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
    /// 这一次调用要结束这一轮（施工 4-14）：这一步几件工具的结果都到了，内核不再请求模型，照常记 `turn.ended`
    /// （`completed`）。只有成功的结果带它，只在要的时候写；老日志、别的结果没有这一格。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub end_turn: bool,
    /// 她发出去的图（施工 X-19，设计 `34-表情包.md` 第五节第 4 条）：工具交的，照交的先后。不发给模型（她看到的是调用和结果
    /// 的字），头画在正文里，通讯平台照它发。没有的不写这一格，老日志字节不变。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sent: Vec<Sent>,
    /// 这件工具给人看的名字，照语言（`zh`、`en`……；施工 9-8 六补）：脚本、MCP 的、扩展的工具派出去那一刻的，头在时间线上
    /// 照连接的语言挑。不发给模型。核心自带的、内核自己写的结果、没写显示名的、老日志没有这一格。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub display_name: BTreeMap<String, String>,
    /// 给头看的结构化结果（施工 W-11，2026-10-17 两个头要的）：一个对象，形状由那件工具定、写在它自己那一页图纸里，例如
    /// `memory_search` 命中的记忆、`kb_read` 那一页正文。不发给模型，内核不看，原样记、原样回放。工具没交的、老日志没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<crate::raw::RawJson>,
}

/// 给人看的说法（`03-事件模型.md` 第三节，施工 4-5 上）：用的是哪一句、换进去的字段。字放在资源目录的
/// `human/` 下，一种语言一份；说法记进日志以后原样回放，换一种界面语言照样换得出字。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Said {
    /// 哪一句：照资源目录里的位置起，内核的以 `core/` 开头，软件包的以 `software/<软件包>/` 开头，
    /// 例如 `core/tool-results/unattended`。
    pub key: String,
    /// 换进去的字段，值都是字符串。没有的不写。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
}

impl Said {
    /// 说 `key` 这一句，没有字段。
    pub fn new(key: impl Into<String>) -> Said {
        Said {
            key: key.into(),
            fields: BTreeMap::new(),
        }
    }

    /// 再换进一个字段。
    #[must_use]
    pub fn with(mut self, field: &str, value: impl Into<String>) -> Said {
        self.fields.insert(field.to_string(), value.into());
        self
    }
}

text_enum!(
    /// 工具调用的结果怎样。
    ToolStatus {
        /// 成功。
        Ok = "ok",
        /// 失败：工具执行了，但是出了错。错在哪，写在给模型看的内容里。
        Error = "error",
        /// 已取消：回合被打断时，还没有结果的调用（`02-内核.md` 第六节）。
        Cancelled = "cancelled",
        /// 被拒绝：执行前被人或守卫拒绝了。谁拒的，看事件的 `by`。
        Denied = "denied",
        /// 已跳过：人急着插话，剩下还没跑的调用不跑了（`02-内核.md` 第六节）。
        Skipped = "skipped",
    }
);

/// `tool.approval_requested`：请人确认一次调用（`02-内核.md` 第六节「确认怎么走」）。
/// `by` 是提问的模块。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequested {
    /// 请人确认的是哪一次调用。
    pub call_id: CallId,
    /// 要的是哪一类访问。收紧成只读时，要写入的当场拦下。
    pub access: Access,
    /// 提的放行规则：选本会话都允许、这个工作区以后都允许时，放行的就是它。写法由提问的模块定，
    /// 内核原样记。没提的，只能选允许这一次或者拒绝。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<RawJson>,
    /// 给头看的：为什么要问。写法由提问的模块定，内核原样记。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<RawJson>,
}

/// `tool.approval_decided`：人对一个请求的决定。`by` 是回答的人。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalDecided {
    /// 决定的是哪一次调用的请求。
    pub call_id: CallId,
    /// 选了哪一项。
    pub decision: Decision,
    /// 拒绝的理由，会写进给她看的结果。只跟着拒绝，没写就没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

text_enum!(
    /// 人对确认请求的决定：问人时的四个选项（`11-权限与沙盒.md` 第二节）。
    Decision {
        /// 允许这一次。
        Once = "once",
        /// 本会话都允许。
        Session = "session",
        /// 这个工作区以后都允许。
        Workspace = "workspace",
        /// 拒绝。拒绝以后她接着干（A13）。
        Deny = "deny",
    }
);

#[cfg(test)]
mod tests;
