//! 回合开始问扩展要加的字（施工 X-16，`docs/blueprint/extensions.md`「回合开始的挂接点」）：协议端点交给会话的端口。会话在
//! 记忆、知识库的挂接点之后问它；问谁、限时、写法对不对由端口管，交回来的都是能记的。
//!
//! 工具调用前的守卫（施工 HK-1，设计 `65-工具挂接点.md` 第三节）也经这个端口：会话在权限策略判完、没拒绝的调用上先查表
//! （[`HooksPort::guards`]），有挂在这次调用上的守卫才问（[`HooksPort::guard`]），照包的编号合好的结论交回来；问谁、限时、
//! 没答的怎么算、拒绝时写给模型的那一句都由端口管。
//!
//! 工具调用后的通知（施工 HK-2，设计 65 第四节）也经这个端口：结果落了盘，会话先查表（[`HooksPort::afters`]），有收它的扩展
//! 才写好一条（[`AfterNote`]）交给端口（[`HooksPort::after`]）。端口只入队、当场交回，不等、不挡：发给谁、队列满了怎么办都由
//! 端口管。
//!
//! 同一个人格的人生会话里她写的「现在在干嘛」（施工 LF-7，`docs/blueprint/life.md`「现在在干嘛」）也经这个端口：回合开始会话
//! 照自己的人格问一次（[`HooksPort::doing`]），端口只查内存，交回写好的那一行；交不交、交过没有由会话照上下文判。

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::ModuleId;
use miyu_kernel::raw::RawJson;
use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::Worded;

/// 同一个人格的人生会话里她写的「现在在干嘛」，写好的一行（施工 LF-7）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoingLine {
    /// 给模型看的那一行：外壳、写下的时刻、下次醒的时刻、她写的那一句，以换行结尾。
    pub text: String,
    /// 认它的记号：写下的时刻（毫秒）。这一段上下文里最近交过的那一块记号一样的，不再交。
    pub key: String,
}

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

/// 工具调用后发给扩展的一条（施工 HK-2，设计 65 第四节第 3 条）：会话照落了盘的那一条写好。
#[derive(Debug, Clone, PartialEq)]
pub struct AfterNote {
    /// 会话编号。
    pub session: String,
    /// 会话的场所编号（`local` 是本机）、关没关着：端口照它判谁碰得到这个会话（同事件流的规矩）。
    pub venue: String,
    /// 会话关没关着（`session.created` 的 `confined`）。
    pub confined: bool,
    /// 这次调用的编号：直接调的是调用编号，脚本里调的是嵌套调用的编号（`call_12_1#3`）。
    pub call_id: String,
    /// 工具名。
    pub tool: String,
    /// 访问类别的写法；目录里没有这件工具的没有。
    pub access: Option<String>,
    /// 结果怎样：直接调的同 `tool.result` 的 `status`，脚本里调的同 `tool.nested` 的。
    pub status: String,
    /// 执行用了多少毫秒；没真执行过的没有。
    pub duration_ms: Option<u64>,
    /// 那条事件的 `by`，照事件流里的写法：被拒的是拒它的那一方。
    pub by: serde_json::Value,
    /// 参数原文，同事件流里的：直接调的是模型写的那一份（`tool_call` 块），脚本里调的是修正过的（`tool.nested`）。
    pub args: String,
    /// 结果：直接调的是 `blocks`，脚本里调的是存成的 blob。
    pub outcome: AfterOutcome,
}

/// 一条通知里的结果，照事件流里的原样（施工 HK-2）：大的只给引用，不另塞字节。
#[derive(Debug, Clone, PartialEq)]
pub enum AfterOutcome {
    /// 直接调的：`tool.result` 的 `blocks`。
    Blocks(serde_json::Value),
    /// 脚本里调的：`tool.nested` 的 `result`（blob 的哈希）、`bytes`；没存下来的没有。
    Stored {
        /// blob 的哈希。
        result: Option<String>,
        /// 那份结果有多少字节。
        bytes: Option<u64>,
    },
}

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

    /// 工具 `tool`、访问类别 `access` 的调用有没有扩展要收 `tool.after`（施工 HK-2）：只查一次内存里的表，不读盘。没有的会话
    /// 不写通知，一点不多。
    fn afters(&self, tool: &str, access: Option<&str>) -> bool {
        let _ = (tool, access);
        false
    }

    /// 发一条 `tool.after`（施工 HK-2）：只入队、当场交回，不等扩展。发给谁、满了丢哪条由端口管。
    fn after(&self, note: AfterNote) {
        let _ = note;
    }

    /// 人格 `persona` 的人生会话里她写的「现在在干嘛」，照会话的时区 `offset` 写好（施工 LF-7）：问的会话 `session` 就是人生
    /// 会话的、人生停着的、没了的、她还没写过的没有。只查内存，不读盘：每个会话每一轮都问。
    fn doing(&self, persona: &str, session: &str, offset: UtcOffset) -> Option<DoingLine>;
}
