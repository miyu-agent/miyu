//! 造会话、载入要交进来的几样（施工 8-8 从 `open.rs` 挪来：那一页长过了行数的上限）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::Permission;
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::time::UtcOffset;
use miyu_policy::features::Features;
use miyu_policy::memory::MemoryScope;
use miyu_policy::preset::{Chosen, PresetFile};
use miyu_policy::{PersonaTexts, Profile};
use miyu_store::index::SessionIndex;
use miyu_store::personas::Personas;
use miyu_store::presets::Presets;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_store::usage::UsageIndex;
use miyu_tool::Shelf;

use crate::config::Configs;
use crate::jobs::Jobs;
use crate::port::Models;
use crate::sandbox::SandboxCache;
use crate::spawn::{Lineage, SessionPort};

/// 预设的几层和这台机器上装了的功能（施工 P-2 下；施工 F-3 上起是功能）：回合开始时照它看预设的文件改了没有。
#[derive(Debug, Clone)]
pub struct PresetPlaces {
    /// 几层。
    pub presets: Presets,
    /// 装了的功能（设计 `30-插件框架.md` 第三节）。
    pub features: Features,
}

impl PresetPlaces {
    /// 照编号 `id` 找叠好的预设文件（载入时放开技能的文件夹用）：找不着、写错了的没有。显示不了图片的（`images` 是假）要图的包
    /// 照旧扣下（施工 X-25），带上这一轮的全局开关 `switches`（施工 X-27）、这个人关了的包 `off`（施工 U23 末上）。
    pub(crate) fn file(
        &self,
        id: &str,
        images: bool,
        (switches, off): (BTreeMap<String, bool>, BTreeSet<String>),
    ) -> Option<PresetFile> {
        let mut file = self
            .presets
            .find(id)
            .ok()?
            .file
            .following(switches)
            .switching_off(off, &self.features);
        if !images {
            file.withheld = self.features.image_packages().clone();
        }
        Some(file)
    }
}

/// 取这一刻装了的软件包（施工 X-11 下）：协议端点给的拿着核心的弱引用，核心没了的交空的。
pub type Installed = Arc<dyn Fn() -> Arc<Vec<miyu_store::packages::Found>> + Send + Sync>;

/// 造一个会话要的。
pub struct Create<'a> {
    /// 数据根。
    pub root: &'a DataRoot,
    /// 资源目录：随核心附带的字从这里读。
    pub resources: &'a ResourceRoot,
    /// 会话编号，照 [`crate::new_id`] 造。
    pub id: SessionId,
    /// 照哪个人格造：编号。无人格的没有（施工 P-4 上）：`persona_texts` 是空的，记忆落在无人格那一间（施工 P-12）。
    pub persona: Option<&'a str>,
    /// 这个人格的字，几层叠好的（施工 P-1 上，`miyu_store::personas`）：造快照用。
    pub persona_texts: PersonaTexts,
    /// 「关于你」（施工 P-9）：这个人写的称呼、更多描述，造会话时照 `core/profile.txt` 拼进 system；两项都空的、场所会话
    /// 没要的是空的（端点定）。
    pub profile: Profile,
    /// 人格的几层（施工 P-1 再补）：回合开始时照它看人格的文件改了没有。
    pub personas: Personas,
    /// 记忆归哪个账号（施工 P-1 上，`Personas::memory_account`）：回合库、记忆日志照它和人格开。
    pub memory_account: AccountId,
    /// 记忆的范围（施工 R-3 下，`memory.md`「范围」）：记进快照，以后照它。子会话不管交的是什么，一律 `off`。
    pub memory_scope: MemoryScope,
    /// 在哪个场所。
    pub venue: VenueId,
    /// 会话的属主：会话、blob 都在他的家目录里。
    pub owner: AccountId,
    /// 开始时的权限。
    pub permission: Permission,
    /// 有没有人能确认（`02-内核.md` 第六节「确认怎么走」第 2 条）。
    pub attended: bool,
    /// 一次性的：`miyu ask` 开的（`22-命令行.md` O2，施工 3-9 下）。
    pub oneshot: bool,
    /// 会话所在的环境：时区、工作目录。
    pub environment: Environment,
    /// 造会话的那个命令的编号：`session.created` 的 `cause`。
    pub command: CommandId,
    /// 谁发的造会话。
    pub by: By,
    /// 给会话造请求模型的端口：驱动的占位取自这个会话的策略快照。
    pub models: &'a dyn Models,
    /// 工具目录的架子：照现在的那一份存下这个会话的工具面（施工 4-1），以后照快照发；提供者的包换了，下一个回合换上
    /// （施工 O-2 中）。执行工具时照现在的那一份找。照会话属主的（施工 U23 中中：成员的叠在核心那一个上，`Shelf::over`）。
    pub tools: Shelf,
    /// 系统的家目录：权限策略照它换 `~`、找工具链目录（施工 4-3 下）。读不出来的是空的。
    pub home: Option<&'a Path>,
    /// 沙盒的助手：这台机器上的沙盒能用才有（核心起来时探的，施工 5-4 上）。权限策略照它判执行命令，执行器照它
    /// 给每次调用写沙盒。
    pub sandbox: Option<&'a Path>,
    /// 沙盒的缓存：属主的那一份在哪、你的 cargo 目录在哪（施工 5-4 下）。核心算不出缓存目录的没有，沙盒里不设工具链的
    /// 变量。
    pub sandbox_cache: Option<SandboxCache>,
    /// 父会话和第几层（施工 7-5）：子会话才有，写进 `session.created`；system 接上子会话的场所说明；到了深度上限的，
    /// 工具面里不给 `agent`。
    pub lineage: Option<Lineage>,
    /// 造子会话、给别的会话发命令的端口（施工 7-5）：会话表交进来，派子代理经它。没有的（测试里自己造的），`agent` 照派
    /// 不了出错。
    pub sessions: Option<Arc<dyn SessionPort>>,
    /// 看软件包的端口（施工 F-10 上）：协议端点交进来，`packages` 照它看。没有的（测试里自己造的），`packages` 说看不了。
    pub packages: Option<Arc<dyn miyu_tool::PackagesPort>>,
    /// 回合开始问扩展要加的字（施工 X-16，`extensions.md`「回合开始的挂接点」）：协议端点交进来。没有的（测试里自己造的）不问。
    pub hooks: Option<Arc<dyn miyu_tool::HooksPort>>,
    /// 执行器的任务表，核心里一张（施工 7-3）：后台命令交给它。
    pub jobs: &'a Arc<Jobs>,
    /// 属主的会话列表的索引（施工 3-8 七补）：日志每落一批，顺手更新这个会话的那一行。没有的（测试里自己造的）不更新。
    pub index: Option<Arc<SessionIndex>>,
    /// 用量汇总（施工 8-15）：日志每落一批，顺手写这一批发出去了的请求；`session_usage` 照它查。没有的（测试里自己造的）
    /// 不写，`session_usage` 照什么都没花答。
    pub usage: Option<Arc<UsageIndex>>,
    /// 从哪取配置（施工 8-4）：回合开始时照它冻结这一轮的配置。没有配置服务的（测试里）给 [`crate::fixed`] 的一份。
    pub configs: Configs,
    /// 核心一份的知识库（施工 K-1 下，`knowledge.md`「怎么走」第六条）：照记忆的账号、人格查；没交的（测试里造的）查不了，
    /// 工具面上没有两件。
    pub knowledge: Option<Arc<miyu_store::knowledge::Knowledge>>,
    /// 核心一份的算向量的（施工 K-2，`knowledge.md`「怎么走」第四条）：知识库照意思找用它，不看人格记忆装没装。没交的只走
    /// 关键词。
    pub vectors: Option<Arc<crate::Vectors>>,
    /// 装了的软件包（施工 X-10 上，`skills.md`「技能」第 2、3 条）：照它和属主家目录里的找技能，列进 system，文件夹放开只读。
    /// 每次取的都是这一刻的（施工 X-11 下：回合开始照它再找一遍，装卸以后跟得上）。没交的（测试里自己造的）不找技能。
    pub installed: Option<Installed>,
    /// 每个场所开着的场所插件（施工 O-60，[`crate::VenuePlugins`]）：一个核心一份。工具面照它筛，回合开始照它重筛，派调用时照它拒。
    pub plugins: crate::VenuePlugins,
    /// 核心一份的记忆（施工 R-2 上、R-3 中，`memory.md`）：主会话每落一批，把结束了的人开的回合放进这个人格的回合库；三件
    /// 工具的端口照它造。没有的（测试里自己造的）不放、三件工具说记忆没开。
    pub memory: Option<Arc<crate::Memory>>,
    /// 会话用哪个模型（施工 8-8）：已经查过的引用，模型或 `@池`（协议的 `session.create` 的 `model`、派子代理时照 `pool`
    /// 或父会话的）。没有的照这时的 `models.chat`。记进 `session.created` 的 `model`。
    pub model: Option<String>,
    /// 会话用哪个预设（施工 P-2 上、中）：已经找好的，编号记进 `session.created` 的 `preset`；子会话照父会话的。工具面照它
    /// 筛，记忆没开的范围一律 `off`，记进快照（[`miyu_policy::PresetPin`]）。测试里自己造的可以没有：全开。
    pub preset: Option<Chosen>,
    /// 明着选了无工具（施工 P-16 下）：一件工具都不给，`preset` 是空的；记进快照、`session.created` 的 `no_tools`。
    pub no_tools: bool,
    /// 预设的几层和装了的软件（施工 P-2 下）：改了预设的文件下一个回合换上。没有的（测试里自己造的）不看。
    pub presets: Option<PresetPlaces>,
    /// 群会话（施工 O-13 中）：`venue.session` 的 `kind` 是 `group` 的有。有的 system 接上群聊的格式说明，快照钉下这时的
    /// 时区和群聊近况最多带几条，群里的人说的渲染成一行一条。
    pub group: Option<GroupSettings>,
    /// 属主是不是管理员（施工 5-12）：场所会话、属主不是管理员的（群会话、陌生人和成员的私聊）只碰得到自己的工作区
    /// （`11-权限与沙盒.md` 第三节「外部身份」）。
    pub owner_is_admin: bool,
    /// 场所显示得了图片吗（施工 X-25）：显示不了的，要图的包整包当没开（[`Chosen::blind`]），记进 `session.created`。
    pub images: bool,
    /// 自动起标题吗（施工 O-43）：假的记进 `session.created`，这个会话从此不自动起标题。
    pub titles: bool,
    /// 整理会话（施工 D-1 上，设计 41）：记进 `session.created`；快照是裸的（没有 system、工具，回合开始不注入事实），之后不随
    /// 人格、预设、工具架换。不是的没有。造的一方另把记忆、知识库、技能、预设都交成关着的。
    pub dream: Option<miyu_kernel::event::Dream>,
    /// 从哪开的（施工 8-33 四补）：`session.terminal` 开的是 `terminal`，记进 `session.created`；别的没有。
    pub origin: Option<String>,
}

/// 照最后一次记下的工作目录（没有的是空的）定实际在哪干活。
pub type Pick<'a> = Box<dyn FnOnce(Option<&str>) -> String + Send + 'a>;

/// 载入的会话在哪干活（施工 V-2 三补）。
pub enum Workplace<'a> {
    /// 给定的：时区、工作目录、加进来的目录。
    Given(Environment),
    /// 照日志里最后一次记下的：载入读日志时顺手认出最后一次记下的工作目录（都没记下的是空的）、加进来的目录，交给 `pick`
    /// 定实际在哪干活（太宽的、落在数据根里的怎么退是会话表的事）。原来会话表在载入之前另读一遍整份日志认它。
    Remembered {
        /// 时区。
        offset: UtcOffset,
        /// 照最后一次记下的工作目录定实际在哪干活。
        pick: Pick<'a>,
    },
}

/// 载入一个会话要的。
pub struct Load<'a> {
    /// 数据根。
    pub root: &'a DataRoot,
    /// 会话的属主。
    pub owner: AccountId,
    /// 人格的几层（施工 P-1 上）：读出快照里的人格以后，照 [`Personas::memory_account`] 算记忆归哪个账号；回合开始时照它
    /// 看人格的文件改了没有（施工 P-1 再补）。
    pub personas: Personas,
    /// 资源目录（施工 P-1 再补）：人格的文件改了，照它重拼快照。
    pub resources: &'a ResourceRoot,
    /// 会话编号。
    pub id: SessionId,
    /// 会话在哪干活（施工 V-2 三补）：给定的，或者照日志里最后一次记下的挑。
    pub place: Workplace<'a>,
    /// 给会话造请求模型的端口：驱动的占位取自这个会话的策略快照。
    pub models: &'a dyn Models,
    /// 工具目录的架子：执行工具时照名字在现在的那一份里找（施工 4-2）。工具面照快照；提供者的包换了，下一个回合换上
    /// （施工 O-2 中）。照会话属主的（施工 U23 中中）。
    pub tools: Shelf,
    /// 系统的家目录：权限策略照它换 `~`、找工具链目录（施工 4-3 下）。读不出来的是空的。
    pub home: Option<&'a Path>,
    /// 沙盒的助手：这台机器上的沙盒能用才有（核心起来时探的，施工 5-4 上）。权限策略照它判执行命令，执行器照它
    /// 给每次调用写沙盒。
    pub sandbox: Option<&'a Path>,
    /// 沙盒的缓存：属主的那一份在哪、你的 cargo 目录在哪（施工 5-4 下）。核心算不出缓存目录的没有，沙盒里不设工具链的
    /// 变量。
    pub sandbox_cache: Option<SandboxCache>,
    /// 造子会话、给别的会话发命令的端口（施工 7-5）：同 [`Create::sessions`]。
    pub sessions: Option<Arc<dyn SessionPort>>,
    /// 看软件包的端口（施工 F-10 上）：同 [`Create::packages`]。
    pub packages: Option<Arc<dyn miyu_tool::PackagesPort>>,
    /// 回合开始问扩展要加的字（施工 X-16，`extensions.md`「回合开始的挂接点」）：协议端点交进来。没有的（测试里自己造的）不问。
    pub hooks: Option<Arc<dyn miyu_tool::HooksPort>>,
    /// 执行器的任务表，核心里一张（施工 7-3）：后台命令交给它，任务编号照日志往后数。
    pub jobs: &'a Arc<Jobs>,
    /// 同 [`Create::index`]。
    pub index: Option<Arc<SessionIndex>>,
    /// 同 [`Create::usage`]。
    pub usage: Option<Arc<UsageIndex>>,
    /// 同 [`Create::configs`]。
    pub configs: Configs,
    /// 同 [`Create::knowledge`]。
    pub knowledge: Option<Arc<miyu_store::knowledge::Knowledge>>,
    /// 同 [`Create::vectors`]。
    pub vectors: Option<Arc<crate::Vectors>>,
    /// 同 [`Create::installed`]：载入时只照它放开技能的文件夹，列出来的那一块照快照。
    pub installed: Option<Installed>,
    /// 同 [`Create::plugins`]：载入以后回合开始照它重筛，派调用时照它拒。
    pub plugins: crate::VenuePlugins,
    /// 同 [`Create::memory`]：载入时照整份事件补上回合库落下的。
    pub memory: Option<Arc<crate::Memory>>,
    /// 同 [`Create::presets`]。
    pub presets: Option<PresetPlaces>,
    /// 同 [`Create::owner_is_admin`]。
    pub owner_is_admin: bool,
}

/// 场所显示不了图片的会话（施工 X-25）：预设里要图的包整包扣下（[`Chosen::blind`]）。没交装了的功能的（测试里造的）照旧。
pub(crate) fn blinded(
    preset: Option<Chosen>,
    images: bool,
    features: Option<&Features>,
) -> Option<Chosen> {
    match (images, features) {
        (false, Some(features)) => preset.map(|chosen| chosen.blind(features)),
        _ => preset,
    }
}

/// 群会话造的时候定的（施工 O-13 中；`recent` 施工 O-13 再补，接入通讯平台要的）：快照钉下，以后不变。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GroupSettings {
    /// 群聊近况最多带几条：没有的只照字节预算（`miyu_policy::RECENT_BUDGET`），有的两个都照、取少的。
    pub recent: Option<u64>,
}
