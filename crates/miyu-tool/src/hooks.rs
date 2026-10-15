//! 回合开始问扩展要加的字（施工 X-16，`docs/blueprint/extensions.md`「回合开始的挂接点」）：协议端点交给会话的端口。会话在
//! 记忆、知识库的挂接点之后问它；问谁、限时、写法对不对由端口管，交回来的都是能记的。

use std::future::Future;
use std::pin::Pin;

/// 这一轮是在哪说的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// 本机：终端、网页。
    Local,
    /// 通讯平台的私聊。
    Private,
    /// 通讯平台的群。
    Group,
}

impl Where {
    /// 协议上的写法。
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Where::Local => "local",
            Where::Private => "private",
            Where::Group => "group",
        }
    }
}

/// 回合开始问一次要交的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnAsk {
    /// 会话编号。
    pub session: String,
    /// 在哪说的。
    pub venue: Where,
    /// 触发这一轮的人话；不是人开的一轮没有。
    pub text: Option<String>,
    /// 这一刻工具面上的工具名：只问工具面上有它的工具的扩展。
    pub tools: Vec<String>,
}

/// 一个包交回来的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inject {
    /// 哪个包：记成事件的 `by`。
    pub package: String,
    /// 事实的类别。
    pub kind: String,
    /// 原文，原样进上下文。
    pub text: String,
}

/// 端口的 future：照包的编号排好的几块。
pub type Injecting<'a> = Pin<Box<dyn Future<Output = Vec<Inject>> + Send + 'a>>;

/// 回合开始问扩展的端口。
pub trait HooksPort: Send + Sync {
    /// 问工具面上有它的工具、声明了 `context.inject` 的扩展，照包的编号排；没答的、超时的、写法不对的不在里面。
    fn turn_start(&self, ask: TurnAsk) -> Injecting<'_>;
}
