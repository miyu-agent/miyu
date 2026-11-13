//! 事件：已经发生的事实，追加进会话的日志（`docs/designs/03-事件模型.md`）。
//!
//! 一条事件在日志里是一行紧凑的 JSON，外壳的写法见第二节「外壳的写法」。
//! 认识的种类读成对应的类型；不认识的，`body` 原样留着。

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::id::{CommandId, EventKind, Seq, TurnId};
use crate::origin::By;
use crate::raw::{self, RawJson};
use crate::time::Timestamp;

mod context;
mod cost;
mod dream;
mod effect;
mod goal;
mod image;
mod job;
mod life;
mod message;
mod model;
mod nested;
mod only;
mod package;
mod peer;
mod question;
mod read;
mod session;
mod shell;
mod tool;
mod transient;
mod turn;
mod venue;

pub use context::{
    CompactTrigger, CompactionPaused, ContextCompacted, ContextInjected, PauseReason, RestoredFile,
};
pub use cost::{Cost, Prices, Real};
pub use dream::DreamEnded;
pub use effect::{
    Effect, FileChanged, FileRead, FileShown, FileTrashed, JobKind, JobMessaged, JobStarted,
    PeerWatch, Todo, TodoStatus, TodoWritten,
};
pub use goal::{
    GoalChanged, GoalEnded, GoalOutcome, GoalPaused, GoalResumed, GoalSet, round_command, round_of,
};
pub use image::ImageDescribed;
pub use job::{ChildReason, ChildReported, JobLines, JobReason, JobReported};
pub use life::{LifeEnded, LifeStarted};
pub use message::{MessageAssistant, MessageUser, MessageWithdrawn, TypedCommand};
pub use model::{
    BlockSpan, CallError, CallResult, ErrorClass, FirstDifference, ForgedCut, ForgedRole,
    MessageRole, ModelCalled, Part, Purpose, Usage,
};
pub use nested::{NestedStatus, ToolNested};
pub use package::{PackageState, State};
pub use peer::{IdleReason, PeerIdle};
pub use question::{Choice, Question, QuestionAnswered, QuestionAsked, Response, fits};
pub use session::{
    CommandRan, ContextCap, Dream, Effort, Forked, Level, LinePin, MetaChanged, Permission,
    PolicyChanged, SessionCreated, SessionRecapped, SessionSealed, TodosCleared, WorkspaceChanged,
};
pub use shell::{ShellRan, ShellReason};
pub use tool::{ApprovalDecided, ApprovalRequested, Decision, Said, Sent, ToolResult, ToolStatus};
pub use transient::{
    ChangeWhy, CompactableChanged, CompactionDone, CompactionProgress, CompactionStarted,
    EffortInUse, EffortSource, ModelChanged, ModelDelta, NestedStarted, PackageStateChanged, Piece,
    RetitleFailed, Retry, Status, TodosChanged, ToolProgress, Transient, TransientBody,
};
pub use turn::{EndReason, TurnEnded, TurnJoined, TurnReverted, TurnStarted, TurnUnreverted};
pub use venue::{
    Forward, ForwardNode, Media, MediaKind, VenueDelivered, VenueMessage, VenueRecalled,
};

/// 一条事件：已经发生的一件事。追加进日志以后不改、不删；撤销和压缩也是追加一条新事件
/// （`03-事件模型.md` 第一节）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// 会话内的序号，从 1 开始，连续递增。
    pub seq: Seq,
    /// 发生的时刻，取自执行器送进来的时钟输入。
    pub at: Timestamp,
    /// 所属回合；不属于任何回合时没有。
    pub turn: Option<TurnId>,
    /// 由谁引起，取自连接，不取自正文。
    pub by: By,
    /// 引起它的命令，用于去重和追踪。
    pub cause: Option<CommandId>,
    /// 事件的种类，连同它自己的内容。
    pub body: Body,
}

/// 事件的种类：每一种写一行，类型和它在 JSON 里的名字。
/// `Body` 本身、`kind`、按种类读、写出去，都照这一张表生成，加一种只加一行。
macro_rules! bodies {
    ($($(#[$doc:meta])* $variant:ident = $kind:literal,)+) => {
        /// 事件的种类，连同它自己的内容。
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Body {
            $($(#[$doc])* $variant($variant),)+
            /// 不认识的种类，包括不认识的 `ext.*`：`body` 原样留着，投影跳过它。
            Unknown {
                /// 外壳里写的种类名。
                kind: EventKind,
                /// 原样的 `body`，写出去一字不差。
                body: RawJson,
            },
        }

        impl Body {
            /// 内核认识的全部种类名，照表里的先后。样本测试拿它查每一种都有样本
            /// （03 第三节「样本文件」）。
            pub const KINDS: &'static [&'static str] = &[$($kind,)+];

            /// 外壳里 `kind` 那一格写的名字。
            pub fn kind(&self) -> &str {
                match self {
                    $(Body::$variant(_) => $kind,)+
                    Body::Unknown { kind, .. } => kind.as_str(),
                }
            }

            /// 已经知道种类时直接照那一种读 `body`，不先截原文（施工 V-11）。不认识的种类原样留着；认识的读不出来报错，原话
            /// 同 [`Body::read`]。
            fn read_from<'de, D: Deserializer<'de>>(kind: &EventKind, d: D) -> Result<Body, D::Error> {
                let read = match kind.as_str() {
                    $($kind => $variant::deserialize(d).map(Body::$variant),)+
                    _ => {
                        return RawJson::deserialize(d).map(|body| Body::Unknown {
                            kind: kind.clone(),
                            body,
                        });
                    }
                };
                read.map_err(|e| D::Error::custom(format!("body of {kind} not readable: {e}")))
            }

            /// 按种类读 `body`。认识的种类读不出来是坏数据，报错写明是哪一种。
            fn read(kind: EventKind, body: RawJson) -> Result<Body, String> {
                let read = match kind.as_str() {
                    $($kind => raw::parse(body.get()).map(Body::$variant),)+
                    _ => return Ok(Body::Unknown { kind, body }),
                };
                read.map_err(|e| format!("body of {kind} not readable: {e}"))
            }
        }

        /// `body` 只写它自己的内容；种类写在外壳的 `kind` 里。
        impl Serialize for Body {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                match self {
                    $(Body::$variant(body) => body.serialize(s),)+
                    Body::Unknown { body, .. } => body.serialize(s),
                }
            }
        }
    };
}

bodies! {
    /// 会话创建。
    SessionCreated = "session.created",
    /// 换了策略快照，或者换了权限。
    PolicyChanged = "session.policy_changed",
    /// 改了标题、置顶。
    MetaChanged = "session.meta_changed",
    /// 换了会话在哪个目录干活（施工 9-7 上）。
    WorkspaceChanged = "session.workspace_changed",
    /// 一句回顾（施工 3-8 四补）。
    SessionRecapped = "session.recapped",
    /// 跑满了轮数，封存了（施工 C-8 下）。
    SessionSealed = "session.sealed",
    /// 人用了一个斜杠命令（施工 O-6）。
    CommandRan = "command.ran",
    /// 人清掉了待办（施工 D-3 再补）。
    TodosCleared = "todos.cleared",
    /// 包的程序直接换上了它在这个会话里的一份状态（施工 TW-1）。
    PackageState = "package.state",
    /// 回合开始。
    TurnStarted = "turn.started",
    /// 照记下的几条并进正在跑的这一轮（施工 O-14 下）。
    TurnJoined = "turn.joined",
    /// 回合结束。
    TurnEnded = "turn.ended",
    /// 撤销了几个回合。
    TurnReverted = "turn.reverted",
    /// 恢复了最近一次撤销的回合。
    TurnUnreverted = "turn.unreverted",
    /// 人发来的消息，或者另一个会话发来的消息。
    MessageUser = "message.user",
    /// 模型一次响应的完整内容，工具调用也在里面。
    MessageAssistant = "message.assistant",
    /// 撤回排着队、她还没听到的消息。
    MessageWithdrawn = "message.withdrawn",
    /// 一个工具调用的结果。
    ToolResult = "tool.result",
    /// 脚本里的一次 `miyu call` 了结了（施工 M-1）。
    ToolNested = "tool.nested",
    /// 请人确认一次工具调用。
    ApprovalRequested = "tool.approval_requested",
    /// 人对确认请求的决定。
    ApprovalDecided = "tool.approval_decided",
    /// 一个在跑的调用请人回答一组题。
    QuestionAsked = "question.asked",
    /// 人对一组题的回答。
    QuestionAnswered = "question.answered",
    /// 注入进上下文的一块事实。
    ContextInjected = "context.injected",
    /// 压缩的检查点。
    ContextCompacted = "context.compacted",
    /// 暂停了自动压缩（施工 6-6 上）。
    CompactionPaused = "context.compaction_paused",
    /// 一次模型请求的记录。
    ModelCalled = "model.called",
    /// 后台命令结束了（施工 7-1）。
    JobReported = "job.reported",
    /// 子会话的回报（施工 7-1）。
    ChildReported = "child.reported",
    /// 监视的后台命令出的一批行（施工 MN-1 上）。
    JobLines = "job.lines",
    /// 人在输入框里用 `!` 打的一条命令跑完了（施工 SH-1 上）。
    ShellRan = "shell.ran",
    /// 等的那个会话空下来了，或者等不到了（施工 C-1）。
    PeerIdle = "peer.idle",
    /// 一张图的转述：看不了图的模型由 `models.vision` 替它看过（施工 8-17）。
    ImageDescribed = "image.described",
    /// 场所里有人撤回了一条消息（施工 O-13 上，桥经 `events.append` 记）。
    VenueRecalled = "venue.recalled",
    /// 她的一句话实际发到了平台上（施工 O-13 上，桥经 `events.append` 记）。
    VenueDelivered = "venue.delivered",
    /// 设了一个长期目标（施工 X-6）。
    GoalSet = "goal.set",
    /// 改了长期目标的原文或上限（施工 X-6）。
    GoalChanged = "goal.changed",
    /// 长期目标停下（施工 X-6）。
    GoalPaused = "goal.paused",
    /// 长期目标接着跑（施工 X-6）。
    GoalResumed = "goal.resumed",
    /// 长期目标结束了（施工 X-6）。
    GoalEnded = "goal.ended",
    /// 整理会话跑完了一趟（施工 DR-1）。
    DreamEnded = "dream.ended",
    /// 这个会话是它的人格的人生会话（施工 LF-5 中）。
    LifeStarted = "life.started",
    /// 这段人生结束了（施工 LF-4 上）。
    LifeEnded = "life.ended",
}

impl Event {
    /// 写成日志里的一行：紧凑的 JSON，字段照图纸的顺序，不带换行。换行由存日志的那一层加。
    ///
    /// # Panics
    ///
    /// 实际不会 panic。serde_json 只在两种情况下写不出来：键不是字符串，或者某个 `Serialize`
    /// 自己报错。事件里这两样都没有。
    pub fn to_line(&self) -> String {
        serde_json::to_string(self)
            .expect("事件里只有字符串、数字和原样的 JSON，写成 JSON 不会失败")
    }

    /// 从日志里的一行读回来。认识的种类读成对应的类型，不认识的 `body` 原样留着。
    ///
    /// # Errors
    ///
    /// 这一行不是 JSON、缺了外壳的字段、某个字段不合写法、认识的种类 `body` 读不出来，
    /// 都返回错误，写明哪里错。
    pub fn from_line(line: &str) -> Result<Event, serde_json::Error> {
        serde_json::from_str(line)
    }

    /// 同 [`Event::from_line`]，只是只解 `only` 里的种类（施工 V-39 上补：判官读群会话日志用）。外壳照旧全解；`only` 里的
    /// 种类、有效历史照种类认的那几种（撤销、恢复、撤回、压缩、回合的开头和结尾、人话、回报、子会话回报、等的会话空了）
    /// 全解；`message.assistant`、`model.called` 只解有效历史撤销时看的 `seen`、`error` 几格，别的格空着；别的种类的
    /// `body` 只过一遍（是合法的 JSON 就行），读成 [`Body::Unknown`]，种类照写、`body` 是 `null`。照先后交给留着一切的
    /// 有效历史（[`crate::history::History::whole`]）算出来留下的事件和全解的一样。
    ///
    /// # Errors
    ///
    /// 读不成的照 [`Event::from_line`] 再读一遍，交回它的错，和全解报的一字不差；它读成了的交它读成的。不要的种类的
    /// `body` 合 JSON、只是不合那一种的写法的，不报（全解报）。
    pub fn from_line_only(line: &str, only: &[&str]) -> Result<Event, serde_json::Error> {
        read::line_only(line, only).or_else(|_| Event::from_line(line))
    }

    /// 不解，照写出去的样子认一行的种类（施工 V-41 上：翻页先扫一遍日志，只解要的那几行）。[`Event::to_line`] 写的每一行
    /// 都以 `{"seq":<数>,"at":"<时刻>","kind":"<种类>"` 打头（外壳照图纸的先后，紧凑、不转义）：对得上的交回种类；对不上的
    /// （字段换了先后、带空白、带转义的，内核写不出来）交回空的，要知道种类得照 [`Event::from_line`] 解。
    ///
    /// 对得上的、这一行又读得成的，认出来的就是它读成的种类：打头的这一格就是外壳的 `kind`，外壳的每一格只许写一遍。
    #[must_use]
    pub fn kind_written(line: &[u8]) -> Option<&str> {
        let rest = line.strip_prefix(b"{\"seq\":")?;
        let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        let rest = rest.get(digits..).filter(|_| digits > 0)?;
        let rest = rest.strip_prefix(b",\"at\":\"")?;
        let rest = &rest[plain(rest)?..];
        let kind = rest.strip_prefix(b"\",\"kind\":\"")?;
        std::str::from_utf8(&kind[..plain(kind)?]).ok()
    }
}

/// 一个 JSON 字符串里没有转义、没有控制字符的那一截有多长：到下一个 `"` 为止；先碰到 `\`、控制字符、到了末尾的没有。
fn plain(text: &[u8]) -> Option<usize> {
    let end = text
        .iter()
        .position(|&byte| byte == b'"' || byte == b'\\' || byte < 0x20)?;
    (text[end] == b'"').then_some(end)
}

/// 写出去的样子。字段的顺序就是图纸上的顺序。
#[derive(Serialize)]
struct LineOut<'a> {
    seq: Seq,
    at: Timestamp,
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn: Option<TurnId>,
    by: &'a By,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<&'a CommandId>,
    body: &'a Body,
}

impl Serialize for Event {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        LineOut {
            seq: self.seq,
            at: self.at,
            kind: self.body.kind(),
            turn: self.turn,
            by: &self.by,
            cause: self.cause.as_ref(),
            body: &self.body,
        }
        .serialize(s)
    }
}

#[cfg(test)]
mod tests;
