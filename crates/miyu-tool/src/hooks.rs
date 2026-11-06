//! 回合开始问扩展要加的字（施工 X-16，`docs/blueprint/extensions.md`「回合开始的挂接点」）：协议端点交给会话的端口。会话在
//! 记忆、知识库的挂接点之后问它；问谁、限时、写法对不对由端口管，交回来的都是能记的。
//!
//! 工具调用前的守卫（施工 HK-1，设计 `65-工具挂接点.md` 第三节）也经这个端口：会话在权限策略判完、没拒绝的调用上先查表
//! （[`HooksPort::guards`]），有挂在这次调用上的守卫才问（[`HooksPort::guard`]），照包的编号合好的结论交回来；问谁、限时、
//! 没答的怎么算、拒绝时写给模型的那一句都由端口管。

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::ModuleId;
use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Worded;

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

/// 工具调用前问一次守卫要交的（施工 HK-1，设计 65 第三节第 3 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardAsk {
    /// 会话编号。
    pub session: String,
    /// 这次调用的编号：直接调的是调用编号，脚本里调的是嵌套调用的编号。
    pub call_id: String,
    /// 工具名。
    pub tool: String,
    /// 修正过的参数：一个 JSON 对象的原文，同执行时的那一份。
    pub args: String,
    /// 访问类别的写法。
    pub access: String,
    /// 这一轮的工作目录。
    pub cwd: String,
    /// 这一轮开口的是不是主人（同 `tool.call` 的 `owner`）。
    pub owner: bool,
    /// 在哪说的。
    pub venue: Where,
}

/// 挂在这次调用上的守卫合起来的结论（施工 HK-1，设计 65 第三节第 4 条）：端口照包的编号合好，最严者胜。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Guarded {
    /// 都放行了。
    Allow,
    /// 有一个要问人，没有拒绝的：编号在前的那一个。
    Ask {
        /// 哪个包问的：写成请求的 `by`。
        module: ModuleId,
        /// 给头看的为什么要问。
        detail: Option<RawJson>,
    },
    /// 有一个拒绝（没答、答得不对的也算）：编号在前的那一个。
    Deny {
        /// 哪个包拒的：写成被拒绝的结果的 `by`。
        module: ModuleId,
        /// 写给模型的那一句（套过模板、转义过）和给人看的说法。
        worded: Worded,
    },
}

/// 问守卫的 future：照包的编号合好的结论。丢掉它就是不等了（打断、回合被取消）：还没答的由端口发 `guard.cancel`。
pub type Guarding = Pin<Box<dyn Future<Output = Guarded> + Send + 'static>>;

/// 回合开始问扩展的端口。
pub trait HooksPort: Send + Sync {
    /// 问工具面上有它的工具、声明了 `context.inject` 的扩展，照包的编号排；没答的、超时的、写法不对的不在里面。
    fn turn_start(&self, ask: TurnAsk) -> Injecting<'_>;

    /// 扩展这一刻要放在会话 `session` 的 system 尾部的那几段（施工 X-20，`system.append`）：键是包的编号；`Some` 是要放的原文，
    /// `None` 是去掉了。没出现的包是没说过，照快照里钉的。
    fn appended(&self, session: &str) -> BTreeMap<String, Option<String>> {
        let _ = session;
        BTreeMap::new()
    }

    /// 工具 `tool`、访问类别 `access` 的调用上有没有挂着守卫（施工 HK-1）：查一次表，不问。没有的照现在的路走，一点不多。
    fn guards(&self, tool: &str, access: &str) -> bool {
        let _ = (tool, access);
        false
    }

    /// 问挂在这次调用上的守卫（施工 HK-1）：几个并行问、各自限时，照包的编号合。表里没有的放行。
    fn guard(&self, ask: GuardAsk) -> Guarding {
        let _ = ask;
        Box::pin(async { Guarded::Allow })
    }
}
