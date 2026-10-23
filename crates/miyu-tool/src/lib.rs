//! 工具的接口和工具目录（`docs/designs/05-内核接口.md` 第六节「工具的规格」、第八节「启用、注册与
//! 目录快照」，施工 4-1）。
//!
//! 工具是内核之外的软件（`10-自带软件.md` B1）：每件工具报出自己的规格，核心起来时登记进工具目录，
//! 登记完就冻结。造会话时，会话照目录把工具面存进策略快照（`03-事件模型.md` E5），以后一直照快照发。
//!
//! - [`Spec`]：一件工具的规格，第一批四格；
//! - [`Tool`]：一件工具：报规格，报一次调用要碰的路径（[`Target`]，施工 4-3 下），执行一次调用（[`Call`] 进、
//!   [`Done`] 出，施工 4-2）；工具执行时经 [`Call::real`] 换路径，换出来的要是权限策略判过的（[`Unchecked`]，施工 5-18）；
//! - [`Catalog`]：工具目录，登记时查重名、名字和参数格式的写法；
//! - [`Log`]：这个会话日志的只读入口（施工 6-4），`history` 用；
//! - [`AgentPort`]：派子代理的端口（施工 7-5），`subagent` 用；
//! - [`MessagePort`]：发话的端口（施工 7-7、C-5），`send_message` 用；
//! - [`JobPort`]：任务端口（施工 7-3），`shell` 把起好的后台命令交给它，`jobs` 经它查、停（施工 7-4）；
//! - [`SessionsPort`]：列会话、只读地开别的会话的日志（施工 C-3、C-4），`sessions`、`history` 用；[`find_session`]
//!   认她写的会话编号；
//! - [`UsagePort`]：查这个会话的用量、金额、上下文（施工 8-15），`session_usage` 用；
//! - [`GoalPort`]：看、设、结束这个会话的长期目标（施工 X-8 上），`goal` 用；
//! - [`picture`]：什么算一张图（施工 4-13 定，3-9 三补挪来）：`read` 读到的、人附上的，都照它认。
//! - [`attachment`]：一个附件是什么（施工 3-9 三补定，C-9 挪来）：人附上的、她带给别的会话的，都照它认。

mod agents;
pub mod attachment;
mod caller;
mod catalog;
mod checked;
pub mod detail;
mod goals;
mod hooks;
mod jobs;
mod knowledge;
pub mod load;
mod log;
mod memory;
mod messages;
mod nested;
mod packages;
pub mod picture;
pub mod private;
mod questions;
pub mod readonly;
mod run;
mod sessions;
mod settings;
mod shelf;
mod stop;
#[cfg(feature = "testkit")]
pub mod testkit;
mod todos;
mod usage;

pub use agents::{
    AgentPort, NotSpawned, Order, SUBAGENT, SUBAGENT_FORMERLY, Spawned, Spawning, is_subagent,
};
pub use caller::{Caller, InUse};
pub use catalog::{
    BASESYSTEM, Catalog, CatalogError, MCP_PREFIX, Problem, mcp_prefix, mcp_service,
};
pub use checked::Unchecked;
pub use goals::{
    BLOCKED_AFTER, GOAL, GoalAnswering, GoalAsk, GoalNow, GoalPort, GoalRefused, GoalSeen,
};
pub use hooks::{HooksPort, Inject, Injecting, TurnAsk, Where};
pub use jobs::{
    Asking, BACKGROUND, Background, Exit, JobError, JobPort, Listed, Output, Process,
    RUN_IN_BACKGROUND, SHELL, Text,
};
pub use knowledge::{
    KB_DELETE, KB_EDIT, KB_HITS, KB_READ, KB_SAVE, KB_SEARCH, KbHit, KbPage, KbRefused, KbSave,
    KbTrashed, KbWritten, KnowledgePort,
};
pub use log::{Log, ReadLog};
pub use memory::{
    FORGET, FoundMemory, FoundTurn, MEMORIES, MEMORY_SEARCH, MemoryPort, Pending, REMEMBER,
    Refused, Remember, Searched, TEXT_CHARS, TURNS,
};
pub use messages::{
    Delivered, MessagePort, NotSent, Recipient, SEND_MESSAGE, SEND_MESSAGE_FORMERLY, Sending,
};
pub use nested::{Calling, Hold, Nested, NestedPort, Schema, Settled};
pub use packages::{Looking, PACKAGE, PACKAGES, PackageConflict, PackageRefusal, PackagesPort};
pub use questions::{ASK_USER, Answering, QuestionPort};
pub use run::{
    Call, CallIds, Done, Effect, Hazard, Picture, Previewing, Progress, Running, Seen, ShownFile,
    Target,
};
pub use sessions::{
    Found, Listing, MainSession, Opening, SESSIONS, SessionsPort, find_session, reaches,
};
pub use settings::SettingsPort;
pub use shelf::{Edition, Shelf};
pub use stop::Stop;
pub use todos::TODOWRITE;
pub use usage::{ContextUse, SESSION_USAGE, Spending, Spent, UsagePort};

use std::collections::BTreeMap;
use std::path::Path;

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;

/// 一件工具的规格（05 第六节）：第一批四格（施工 4-1）。显示名、摘要随施工 4-5；默认超时、场所、
/// 组，用得上时再加。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// 工具名：英文，稳定不变，模型照它调。只用英文字母、数字、`_`、`-`，1 到 64 个字符。
    pub name: String,
    /// 给模型看的说明，英文，原样进 tools 数组。
    pub description: String,
    /// 参数的 JSON Schema，原样进 tools 数组，一个字节不改：必须是 `{"type":"object",…}`。
    pub parameters: RawJson,
    /// 访问类别：权限策略、能不能和别的一起跑，都看它。
    pub access: Access,
    /// 要先加载（施工 L-1，`25-工具加载.md` W9）：登记的一方说的。是的，工具面上只给名字、说明和空的参数外壳，她用之前
    /// `load_tools` 拿完整的格式；不是的全量。四个来处各写一处：内置工具的说明文件、脚本开头、扩展 `provide`、MCP 服务。
    pub need_load: bool,
    /// 脚本里能调（施工 M-2，`39-脚本调工具.md` 第三节）：登记的一方说的默认，人在个人设置里能改（`clicall.*`）。内置工具、
    /// 软件包、扩展、脚本工具默认关，MCP 默认开（脚本施工 M-2 三补改）；访问类别不是读、联网、执行的，写了也调不成（内核拦）。
    pub clicall: bool,
    /// 脚本里调到时交回的结构化结果（`Done.data`）的样子（施工 M-4，`39-脚本调工具.md` 第九节第 3 条）：一行紧凑的字，例如
    /// `{"hits":[{"kb","path","snippet"}]}`，给她看、`miyu call --schema` 印。只有交结构化结果的写；内置工具的说明文件、扩展
    /// `provide` 写，MCP、脚本工具没有。
    pub result: Option<String>,
    /// 场所插件（施工 O-60，`venues.md`「场所插件」）：带了的，场所会话里只有那个场所开着它才给，本机会话照旧。只有扩展
    /// `provide` 写；内置工具、脚本工具、MCP 没有。
    pub plugin: Option<String>,
}

/// 提供者的工具给哪种会话（施工 O-2 上，`docs/blueprint/providers.md`）：本机的、通讯平台的私聊、群。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Venues {
    /// 本机的会话。
    pub local: bool,
    /// 通讯平台的私聊。
    pub private: bool,
    /// 群。
    pub group: bool,
}

impl Venues {
    /// 照会话种类的名字拼一份（清单的 `venues`、脚本开头的 `Venues`，读的时候查过了）：不认识的不算。
    pub fn of(names: &[String]) -> Venues {
        let mut venues = Venues::default();
        for name in names {
            match name.as_str() {
                "local" => venues.local = true,
                "private" => venues.private = true,
                "group" => venues.group = true,
                _ => {}
            }
        }
        venues
    }
}

/// 一件工具。
pub trait Tool: Send + Sync {
    /// 它的规格。
    fn spec(&self) -> &Spec;

    /// 给哪种会话（施工 O-2 上）：提供者的工具照它挑。核心自带的没有，照工具面自己的规矩挑（`miyu-session` 的 `Agents::face`）。
    fn venues(&self) -> Option<Venues> {
        None
    }

    /// 给人看的名字，照语言（`zh`、`en`……；施工 9-8 六补）：脚本、MCP 的、扩展的工具交，执行器派它那一刻取下、随结果记进
    /// 日志，头照连接的语言挑。核心自带的不交，照核心的字（`resources/core/human/`）。默认空。
    fn display_name(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// 给人看的名字后面跟哪一个参数的值（施工 S-6 上，`protocol.md` 的 `human.get` 第 4 条）：扩展登记时写的，是参数格式里的
    /// 一个名字；核心照它放进 `human.get` 那件工具的 `subject`。核心自带的照核心的字，不交。默认没有。
    fn subject(&self) -> Option<String> {
        None
    }

    /// 这次调用要碰的路径、是读是写：照参数算，不碰磁盘（施工 4-3 下）。换成真实的位置、查边界是执行前的
    /// 链的事。默认一条都没有，例如执行命令。参数不对的，也交回空的：执行时再报错。
    fn targets(&self, _call: &Call) -> Vec<Target> {
        Vec::new()
    }

    /// 这次调用要在沙盒外跑（施工 D-4）：照参数算，不碰磁盘。权限策略照它判（工作区问人、只读拒绝），执行器照它不写
    /// 沙盒的规格：工具自己从不决定套不套沙盒。默认否。
    fn outside_sandbox(&self, _call: &Call) -> bool {
        false
    }

    /// 它的文件（施工 X-37 补，原来的 `linked`）：管理员放行出沙盒的，判的那一刻照这个文件的真实位置看，落在管理员管得住的
    /// 地方、又不在她这一轮写得了的地方的才算放行（链接、所在的目录是链接，都能指到她改得了的地方）。工具自己不判：它不知道
    /// 数据根、账号在哪。默认没有：不是一个文件的工具没有这回事。
    fn file(&self) -> Option<&Path> {
        None
    }

    /// 这次调用认出来的高危（施工 5-17）：照参数算，不碰磁盘。权限策略照它在工作区、完全放开两级问人（说明多 `hazard`，不提
    /// 规则），灭顶的直接拒；只读不看它。默认没有。
    fn hazard(&self, _call: &Call) -> Option<Hazard> {
        None
    }

    /// 问人时交给说明（`detail`）的几格，键和值（施工 D-4）：照参数算，例如 `shell` 的 `title`、`command`。权限策略并进
    /// 它自己写的那几格。参数不对的交回空的。默认空。
    fn asking(&self, _call: &Call) -> Vec<(&'static str, String)> {
        Vec::new()
    }

    /// 问人之前先看一眼（施工 F-10 下，`docs/blueprint/tools/package.md`）：权限策略要问人时，执行器带着端口调它（只有端口，
    /// 没有沙盒、日志这些）。成了的整份放进确认请求的说明，键 `preview`，人看到的就是日志里记下的；不成的不问，交回的那一份
    /// 照拒绝交给她。默认不看。
    fn preview(&self, _call: Call) -> Option<Previewing<'_>> {
        None
    }

    /// 执行一次调用：执行器在它自己的任务里跑交回的 future，执行中的输出交给 `progress`。叫停有两种：
    /// 「叫它停」举 [`Call::stop`] 的旗，等它交回来（施工 4-9 再补一）；「掐掉」丢掉这个 future（施工 4-2）。
    fn run(&self, call: Call, progress: Progress) -> Running<'_>;

    /// 它以前的名字（施工 7-5 再补）：改过名的工具，改名以前造的会话快照里冻着旧名字（工具面是前缀，不能变），她照旧名字调，
    /// 工具目录照旧名字也找得到这一件（[`Catalog::get`]）。旧名字不进 [`Catalog::specs`]：新造的会话工具面上只有现在的名字。
    /// 默认没有。
    fn formerly(&self) -> &'static [&'static str] {
        &[]
    }

    /// 后台运行关着的会话里用的说明（施工 T-1 下）：在后台跑的工具换一种说法，`subagent` 有（派出去等它报回来）。别的没有，
    /// 照规格的。
    fn foreground_description(&self) -> Option<&str> {
        None
    }

    /// 说明末尾讲脚本调工具的那一段（施工 M-4，设计 `39-脚本调工具.md` 第九节）：这个会话的工具面上有脚本里能调的工具时，组
    /// 工具面的一方把它接在说明后面。`tools` 是能调的名单（一个 MCP 服务的几件并成 `mcp__<服务>__*`），`shapes` 是有结构化结果
    /// 的那几件的样子（`名字 样子`，`; ` 隔开；一件都没有的是空的）。只有 `shell` 有，别的没有。
    fn clicall_section(&self, _tools: &str, _shapes: &str) -> Option<String> {
        None
    }
}
