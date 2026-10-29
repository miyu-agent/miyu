//! 执行工具要的那一份（施工 4-4 上起；从 `tools.rs` 挪出来，施工 O-2 三补）：开会话、载入会话时拼好，交给 [`super::Tools::new`]。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::id::{SessionId, VenueId};
use miyu_kernel::time::UtcOffset;
use miyu_store::blob::Blobs;
use miyu_tool::{Log, Seen, Shelf};

use crate::agents::Agents;
use crate::job_ids::JobIds;
use crate::lettering::Lettering;
use crate::sandbox::SandboxCache;
use crate::usage::Ledger;

/// 执行工具要的：工具目录、替工具写的两句、系统的家目录（施工 4-4 上，交给每次调用）。
pub(crate) struct ToolKit {
    /// 这个会话（施工 O-2 上）：每次调用带上，提供者的工具照它发 `tool.call`。
    pub(crate) session: SessionId,
    /// 关起来的会话（施工 5-12 下）：命令的沙盒读写都只限工作区。
    pub(crate) confined: bool,
    /// 会话的场所（施工 X-43）：每次调用带上，交给脚本（`MIYU_CALLER_VENUE`）。
    /// 每个场所开着的场所插件（施工 O-60）：派调用时这一刻没开的当目录里没有。
    pub(crate) plugins: crate::VenuePlugins,
    pub(crate) venue: VenueId,
    /// 会话日志的目录（施工 E-M）：属主是管理员的才交给命令（`MIYU_SESSION_LOG`），别人的 `None`。
    pub(crate) session_log: Option<PathBuf>,
    /// 工具目录的架子：执行时照现在的那一份找（施工 O-2 中）。
    pub(crate) catalog: Shelf,
    /// 替工具写的两句，和权限策略同一份（施工 P-1 三补：换快照时跟着换）。
    pub(crate) lettering: Arc<Lettering>,
    /// 系统的家目录。
    pub(crate) home: Option<PathBuf>,
    /// Miyu 的数据根：交给工具，往下走目录的走到这里跳过（施工 4-4 下）。
    pub(crate) data_root: PathBuf,
    /// 这个会话的 blob：效果里改前改后的内容存进这里（施工 4-6 上）。
    pub(crate) blobs: Blobs,
    /// 她看过的文件：新会话是空的，载入的从日志里重建（施工 4-6 上）。
    pub(crate) seen: Seen,
    /// 沙盒的助手：这台机器上的沙盒能用才有（核心起来时探的，施工 5-4 上）。
    pub(crate) sandbox: Option<PathBuf>,
    /// 沙盒的缓存：工具链的缓存用沙盒自己的一份（施工 5-4 下）。核心算不出缓存目录的没有。
    pub(crate) sandbox_cache: Option<SandboxCache>,
    /// 这个会话日志的只读入口：交给每次调用，`history` 用（施工 6-4）。
    pub(crate) log: Log,
    /// 会话的时区：开会话时的环境里的（施工 6-4）。
    pub(crate) offset: UtcOffset,
    /// 这个会话的任务编号（施工 7-5）：从日志里用过的最大编号往下数，几次调用一起跑的各领各的。
    pub(crate) job_ids: Arc<JobIds>,
    /// 派子代理要的（施工 7-5）：会话表交进来了端口才有。
    pub(crate) agents: Option<Arc<Agents>>,
    /// 用量汇总里的这个会话（施工 8-15）：`session_usage` 的端口照它造。没开汇总的没有。
    pub(crate) ledger: Option<Ledger>,
    /// 能不能问人（施工 D-2，[`Agents::asks`]）：能的每次调用给一个提问的端口。
    pub(crate) asks: bool,
    /// 有没有长期目标（施工 X-8 上，[`Agents::lists_sessions`]）：本机的主会话每次调用给一个目标的端口。
    pub(crate) goals: bool,
    /// 记忆（施工 R-3 中）：主会话、核心交了记忆的才有，每次调用照它造记忆的端口。
    pub(crate) memory: Option<crate::memory::Calls>,
    /// 知识库（施工 K-1 下，`knowledge.md`「怎么走」第六条）：核心交了知识库的会话都有，每次调用照它造端口；查不查得到在
    /// 端口里照那一刻判。
    pub(crate) knowledge: Option<Arc<crate::knowledge::Shelves>>,
    /// 看软件包的端口（施工 F-10 上）：协议端点交了的才有，每次调用原样带上。
    pub(crate) packages: Option<Arc<dyn miyu_tool::PackagesPort>>,
    /// 定时和循环的端口（施工 J-2）：协议端点交了的才有，每次调用原样带上。
    pub(crate) schedules: Option<Arc<dyn miyu_tool::SchedulePort>>,
    /// 回合开始问扩展要加的字（施工 X-16）。
    pub(crate) hooks: Option<Arc<dyn miyu_tool::HooksPort>>,
    /// 开着的技能文件夹（施工 X-10 中）：沙盒藏着数据根，照它单独放开只读。
    pub(crate) shelves: Vec<std::path::PathBuf>,
    /// 这个会话的附件目录（施工 3-9 六补）：沙盒里单独放开只读。
    pub(crate) attachments: PathBuf,
    /// 工作目录太宽时退回的默认工作区（施工 5-14）：沙盒能写的照它；没有的照工作目录。
    pub(crate) workplace: Option<crate::default_workspace::DefaultWorkspace>,
    /// 开着的技能给人看的名字（施工 9-8 五补）：读它们的 `SKILL.md` 时记进效果。
    pub(crate) names: crate::skills::Names,
    /// 会话用的预设的编号（施工 F-10 下下）：每次调用带上，她装的包在这一份预设里打开。
    pub(crate) preset: Option<String>,
}

impl super::Tools {
    /// 系统的家目录：把头报上来的 `~/…` 换成真实的路径用（施工 P-14 的项目指令）。认不出的没有。
    pub(crate) fn home(&self) -> Option<&std::path::Path> {
        self.home.as_deref()
    }
}
