//! 场所的事件（施工 O-13 上，`docs/blueprint/chat.md` 第七条第 2 条）：`message.user` 的 `venue` 格，桥记的 `venue.recalled`、
//! `venue.delivered`。内核不解读平台的编号，原样记；群聊近况怎么渲染它们在组装那一层（O-13 下）。合并转发的内容（施工 O-34）
//! 也原样记在 `venue.forwards` 里，怎么写成几行在组装那一层（`miyu-assemble` 的 `forward.rs`）。

use serde::{Deserialize, Serialize};

use crate::id::{ContentHash, ExternalId, SessionId, TurnId};
use crate::text_enum::text_enum;
use crate::time::Timestamp;

/// `message.user` 的 `venue`：通讯平台上的一条消息。旁听的（`ambient`）只记下，不开回合，回合进行中也不排进这一轮。
/// 假的、空的、没写的不写。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueMessage {
    /// 平台给的消息编号。
    pub msg: String,
    /// 引用的那一条的平台编号。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// 发的人此刻在这个场所里叫什么（群名片，没有的用昵称）。名字会变，每条消息各记各的。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 她此刻在这个场所里叫什么（施工 E-1 修三，`docs/blueprint/venues.md`「场所的格」）：群名片，空的用昵称，私聊也是昵称。
    /// 名字会变，每条消息各记各的；记忆认她的时候和人格的名字一起算（`miyu-session` 的 `memory/her.rs`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub my_name: Option<String>,
    /// 这个场所此刻叫什么（施工 E-4 上，`docs/blueprint/venues.md`「场所的格」）：群名，私聊不写。名字会变，每条消息各记各的；
    /// 只给记忆用（记下时存进 `ext.memory.saved` 的 `place`），渲染她上下文的地方不读它。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// @ 了谁：平台身份。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<ExternalId>,
    /// @ 了她。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mentions_me: bool,
    /// @ 了全体成员：不算 @ 了她。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mentions_all: bool,
    /// 带的图、文件、语音、视频、表情：只记编号，要看时再由桥去取，不进内容块。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<Media>,
    /// 合并转发的内容（施工 O-34，`docs/blueprint/venues.md`「场所的格」）：照正文里转发的先后，桥取出来的。没有的不写，以前记的
    /// 没有，字节不变。写法、上限由收的一方查（`miyu-endpoint` 的 `venues/message.rs`），这里原样记。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forwards: Vec<Forward>,
    /// 旁听：不冲她来的，只记下。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ambient: bool,
    /// 睡着时收到的：不进群聊近况。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub asleep: bool,
    /// 补拉回来的（施工 O-40，设计 18 第十一节）：桥断线时漏掉、重连以后从平台补回来的，只进聊天记录：同睡着时收到的，不进
    /// 群聊近况、判官看的记录，带着 `ambient`。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub backfill: bool,
    /// 平台给的时刻（施工 O-40）：补拉回来的比记下的时刻早，`history` 照它显示。没写的照记下的时刻。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent: Option<Timestamp>,
    /// 渲染这一条时写不写发的人的平台身份（施工 O-13 中）：桥照这时的场所规则每条带上，规则改了从下一条起照新的。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show_ids: bool,
}

/// 一条消息带的一样东西。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
    /// 哪一种。
    pub kind: MediaKind,
    /// 平台给的编号。
    pub id: String,
    /// 文件的文件名、表情的字；语音、视频没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 多少字节（施工 O-33）：平台说了的才有，群里的一行照它写成人看得懂的大小（`miyu-assemble` 的 `size.rs`）。以前记的
    /// 没有，照旧不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// 原图已经当图片块交进来了（施工 O-35，`docs/blueprint/venues.md`「场所的格」）：桥照它交了的那几样写真。O-35 起造的私聊
    /// 会话写记号时跳过它们，编号照旧连着数（和 `fetch_media` 的 `index` 对得上）；它们不算内容（原图在内容块里，
    /// `session/send.rs`）。假的不写，以前记的没有，字节不变。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shown: bool,
    /// 在合并转发里的编号（施工 O-61，`docs/blueprint/venues.md`「场所的格」）：桥照 `fetch_media` 的数法记的（同一条消息里
    /// 接着它自己带的东西往下数，照展开的先后，套着的转发也算），核心只照记的写、不自己数，重了是桥的事。只在转发里有，
    /// 转发外面的写了收的一方拒。没有的不写，以前记的没有，字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<std::num::NonZeroU32>,
}

/// 一个合并转发（施工 O-34）：桥放进来的几条，和没放进来的条数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forward {
    /// 放进来的几条，照转发里的先后。
    pub nodes: Vec<ForwardNode>,
    /// 桥没放进来的条数：只在大于 0 时有（写 0 的收的一方拒）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub more: Option<u64>,
}

/// 合并转发里的一条（施工 O-34）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardNode {
    /// 转发里那个人当时的名字（桥照节点的发的人取：群名片、昵称）。不可信，渲染时照模板的规矩转义。
    pub name: String,
    /// 那一条的正文，可以是空的（只带了东西的）。不可信，渲染时转义成一行。
    pub text: String,
    /// 带的东西，写法同 [`VenueMessage::media`]：带了 `index` 的（施工 O-61）记号标那个编号，照它用平台工具取。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<Media>,
    /// 套在这一条里的合并转发，同样的形状。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forwards: Vec<Forward>,
}

text_enum!(
    /// 带的东西是哪一种。
    MediaKind {
        /// 图。
        Image = "image",
        /// 文件。
        File = "file",
        /// 语音。
        Voice = "voice",
        /// 视频。
        Video = "video",
        /// 表情（平台的小黄脸、商城表情都算）。
        Sticker = "sticker",
    }
);

/// `venue.recalled`：有人撤回了一条消息。不叫 `withdrawn`：内核的 `message.withdrawn` 是排着队的消息退回给头。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueRecalled {
    /// 被撤的那一条的平台编号。
    pub msg: String,
    /// 谁撤的。
    pub by: ExternalId,
}

/// `venue.delivered`：她的一句话实际发到了平台上（出站链洗过的）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueDelivered {
    /// 哪条线：主线或支线的会话编号。
    pub line: SessionId,
    /// 哪一轮。
    pub turn: TurnId,
    /// 回的是谁。
    pub to: Vec<ExternalId>,
    /// 平台给的编号。
    pub msg: String,
    /// 发出去的正文。
    pub text: String,
    /// 带的图的内容哈希。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ContentHash>,
}

#[cfg(test)]
mod tests;
