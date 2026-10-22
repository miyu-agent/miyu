//! 快照的类型，和它的字节（`docs/designs/03-事件模型.md` E5「写法」）：一个紧凑的 JSON，字段的先后
//! 就是结构体里的先后，同样的内容字节一定一样。装的是发请求要用的全部：人格、拼好的 system、随核心
//! 附带的字、几样开关。模型和供应商不在里面：同一份快照可以交给不同的端点，发请求时才定。

use std::collections::BTreeMap;

use miyu_assemble::{DefaultAssembler, Stable, Texts};
use miyu_drivers::DriverTexts;
use miyu_kernel::estimate::Flat;
use miyu_kernel::event::{Permission, SessionCreated};
use miyu_kernel::id::{AccountId, ContentHash, VenueId};
use miyu_kernel::session::{Compaction, Notes, Policy};
use miyu_kernel::tool::{ToolTextSources, ToolTexts};
use serde::{Deserialize, Serialize};

use crate::drivers::DriverPlaceholders;
use crate::facts::FactTexts;
use crate::forward::ForwardTexts;
use crate::group::GroupChat;
use crate::harness::HarnessTexts;
use crate::jobs::{JobNumbers, JobTexts};
use crate::pause::PauseNumbers;
use crate::peers::{PeerNumbers, PeerTexts};
use crate::persona::Demo;
use crate::preset::PresetPin;
use crate::private_media::PrivateMediaTexts;
use crate::rebuild::{RebuildNumbers, RebuildTexts};
use crate::recap::{RecapNumbers, RecapTexts};
use crate::shorten::{ShortenNumbers, ShortenTexts};
use crate::title::{TitleNumbers, TitleTexts};
use crate::tools::{self, ToolEntry};
use crate::vision::VisionTexts;

/// 一份策略快照。字段的先后就是字节里的先后：改了先后，快照的字节就变了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// 人格的编号，就是资源目录里 `personas/` 下那一层目录的名字。无人格的会话没有（施工 P-4 上，2026-10-08 项目主人：
    /// 「人格允许为空」）：不写，带人格的快照字节和以前一样。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    /// 拼好的 system（`26-提示词.md` 第四节）。
    pub system: String,
    /// 示范对话（施工 P-1 上）：组装时排在 system 后面、历史前面。没有的不写：以前造的快照、没有示范对话的人格，字节和
    /// 以前一样。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demos: Vec<Demo>,
    /// 工具面（施工 4-1）：照名字排好，每件带访问类别。一件都没有的不写：没有工具的快照，字节和以前
    /// 一样，M3 造的会话照旧读得回来、哈希不变。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolEntry>,
    /// 随核心附带的字。
    pub core: CoreTexts,
    /// 一个回合最多请求几次模型；没有就是不限，默认不设（`02-内核.md` 第六节「工具怎么调、下一步怎么走」）。
    pub step_limit: Option<u32>,
    /// 有没有人能确认：有确认界面的头有；`miyu ask` 一律没有（`22-命令行.md` 第三节，`02-内核.md` 第六节「确认怎么走」）。
    pub attended: bool,
    /// 有计划的重启打断了一轮，再起来时连着接着干几次（`02-内核.md` 第六节「载入、崩溃、重启」）。
    pub resumes: u32,
    /// 压缩用的数（施工 6-2 上）。以前造的快照里没有，读成没有：那些会话不主动压。没有的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionNumbers>,
    /// 任务用的数（施工 7-6）。以前造的快照里没有，读成没有：照出厂的数截回报。没有的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs: Option<JobNumbers>,
    /// 回顾用的数（施工 3-8 四补）。以前造的快照里没有，读成没有：不做回顾。没有的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recap: Option<RecapNumbers>,
    /// 起标题用的数（施工 3-8 五补）。以前造的快照里没有，读成没有：不起标题。没有的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<TitleNumbers>,
    /// 防刷屏的数（施工 C-2，`peers.rs`）。以前造的快照里没有，读成没有：照出厂的数。没有的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<PeerNumbers>,
    /// 记忆的范围（施工 R-3 下，`memory.rs`）：`persona`、`session`、`off`。以前造的快照里没有，读成跟着人格；没有的不写，
    /// 旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    /// 角色扮演提示拼好的一块（施工 P-1 补，`compose.rs`）：回合开始时隔几轮注入。没有的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminder: Option<String>,
    /// 人格三份字的指纹（施工 P-1 再补，`compose.rs`）：回合开始时执行器照它认出人格的文件改了。以前造的没有：不换。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_digest: Option<ContentHash>,
    /// 会话的预设（施工 P-2 中，`compose.rs`）：编号，和造会话时装了、这个预设没开的软件。工具面、记忆的范围已经照它筛过，
    /// 换人格重拼时照它去掉角色扮演提示、写「装了没开」那一行。以前造的没有：全开。没有的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<PresetPin>,
    /// 明着选了无工具（施工 P-16 下）：工具面是空的、`preset` 没有，回合开始重新筛也照它。和以前造的没有预设（不筛）分开。
    /// 没选的不写，旧快照的字节不变。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_tools: bool,
    /// 群会话（施工 O-13 中，`group.rs`）：造会话时钉下的时区和空的一条写什么，群里的人说的照它渲染成一行一条。私聊、本机的
    /// 没有：不写，字节和以前一样。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupChat>,
    /// 后台运行关着（施工 T-1 上，设计 30 第三节第 7 条）：这个会话没有后台，`shell` 不放到后台、执行器不给任务端口。照预设
    /// 定；开着的、以前造的不写，字节和以前一样。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub foreground: bool,
    /// 她查得到的知识库那一块拼好的原文（施工 K-1 补，`compose.rs`）：system 第 4 块；回合开始时照它比，变了换快照。没有库、
    /// 没有知识库那两件工具的没有：不写，字节和以前一样。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knowledge: Option<String>,
    /// 她用得上的技能那一块拼好的原文（施工 X-10 上，`compose.rs`）：system 第 4 块里排在知识库前面，造会话时钉下，会话中途
    /// 不改。没有技能、工具面上没有 `read` 的没有：不写，字节和以前一样。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<String>,
    /// 「关于你」那一块拼好的原文（施工 P-9，`compose.rs`）：system 里接在人设、场所说明后面，造会话时钉下，会话中途不改
    /// （换人格、换预设也照旧）。没写称呼和更多描述的、场所会话没要的没有：不写，字节和以前一样。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// 项目的附加提示词那一块拼好的原文（施工 H-3，`project.rs`）：接在「关于你」后面，会话挪进挪出项目、项目改了提示词，下一轮
    /// 换上。不在项目里、没写的没有：不写，字节和以前一样。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// 扩展放在 system 尾部的那几段（施工 X-20，`appended.rs`）：包的编号到原文，造会话时钉下，扩展改了下一轮换上。没有的
    /// 不写，字节和以前一样。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub appended: BTreeMap<String, String>,
    /// 裸的（施工 D-1 上，设计 `41-整理会话.md`，[`Snapshot::bare`]）：整理会话用，回合开始不注入事实。不是的不写，字节和以前
    /// 一样。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bare: bool,
}

/// 随核心附带的字（`resources/core/`），原文照抄，行尾的换行也算（`26-提示词.md` 第八节）。
/// 装进快照：核心升级改了这些字，老会话照样逐字节重现当时的请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreTexts {
    /// 压缩检查点的开头（`checkpoint-open.txt`）。
    pub checkpoint_open: String,
    /// 摘要的收尾（`checkpoint-close.txt`）。
    pub checkpoint_close: String,
    /// 包装的结尾（`checkpoint-end.txt`，施工 6-5 从 close 里拆出来）。以前造的快照里没有，读成空的：那时的 close 里本来
    /// 就带着那一句，拼出来一字不差。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub checkpoint_end: String,
    /// 回合没走完的几句（`turn-ended/`）。
    pub turn_ended: TurnEndedTexts,
    /// 事实的模板（`facts/`）。
    pub facts: FactTexts,
    /// 内核替工具写给模型的几句（`tool-results/`）。
    pub tool_results: ToolResultTexts,
    /// 驱动的几句占位（`drivers/`）。
    pub drivers: DriverPlaceholders,
    /// 权限策略拒绝时写给她的两句（`permissions/`，施工 4-3 下）。这一格以前造的会话里没有，读成空的：那时
    /// 的会话没有工具，用不到它们。
    #[serde(default)]
    pub permissions: PermissionTexts,
    /// 压缩的几句（`compaction/`，施工 6-2 上）。以前造的快照里没有，读成没有，那些会话不主动压；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionTexts>,
    /// 两种回报的写法（`jobs/`，施工 7-2）。以前造的快照里没有，读成没有：那些会话派不出任务；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs: Option<JobTexts>,
    /// 别的 harness 发来的话的标签（`harness/`，施工 7-10）。以前造的快照里没有，读成没有：那种话照人的话原样渲染；没有的
    /// 不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<HarnessTexts>,
    /// 别的会话发来的话的标签（`peers/`，施工 C-2）。以前造的快照里没有，读成没有：那种话照人的话原样渲染；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<PeerTexts>,
    /// 回顾的字（`recap/`，施工 3-8 四补）。以前造的快照里没有，读成没有：不做回顾；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recap: Option<RecapTexts>,
    /// 起标题的字（`title/`，施工 3-8 五补）。以前造的快照里没有，读成没有：不起标题；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<TitleTexts>,
    /// 转述一张图的字（`vision/`，施工 8-17）。以前造的快照里没有，读成没有：不转述；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision: Option<VisionTexts>,
    /// 合并转发那几行的字（`venues/forward/`，施工 O-34，`forward.rs`）。以前造的快照里没有，读成没有：收到转发也不写那几行；
    /// 没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward: Option<ForwardTexts>,
    /// 私聊里带的东西的记号要用的字（`venues/voice.txt`，施工 O-35，`private_media.rs`）。以前造的快照里没有，读成没有：私聊照
    /// 以前渲染；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_media: Option<PrivateMediaTexts>,
    /// 要先加载的工具用的字（`loading/`，施工 L-1）。以前造的快照里没有，读成没有：那时还没有要先加载的工具；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loading: Option<crate::loading::LoadingTexts>,
}

impl Snapshot {
    /// 规范的字节：紧凑的 JSON，字段照结构体的先后。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：快照里只有字符串、数字和开关，写成 JSON 不会失败。
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("快照里只有字符串、数字和开关，写成 JSON 不会失败")
    }

    /// 内容哈希：规范字节的 SHA-256。存成 blob 用的、`session.created` 记的，都是它。
    pub fn hash(&self) -> ContentHash {
        ContentHash::of(&self.to_bytes())
    }

    /// 从字节读回来。
    ///
    /// # Errors
    ///
    /// 不是这个版本写的快照，或者字节坏了。
    pub fn from_bytes(bytes: &[u8]) -> Result<Snapshot, SnapshotError> {
        serde_json::from_slice(bytes).map_err(|error| SnapshotError(error.to_string()))
    }

    /// 造会话的那一条：属主、场所、这份快照的哈希、开始时的权限（`03-事件模型.md` 第三节）。工作目录、模型（施工 8-8）由
    /// 造会话的一方填上。造出来的是主会话：父会话、第几层没有（子会话随 M7，`agents.md`）。
    pub fn session_created(
        &self,
        owner: AccountId,
        venue: VenueId,
        permission: Permission,
    ) -> SessionCreated {
        SessionCreated {
            owner,
            venue,
            policy: self.hash(),
            permission,
            oneshot: false,
            cwd: None,
            parent: None,
            depth: None,
            workspace_from: None,
            model: None,
            persona: self.persona.clone(),
            preset: None,
            no_tools: self.no_tools,
            forked: None,
            images: true,
            titles: true,
            dream: None,
            origin: None,
        }
    }

    /// 照快照造出内核的策略：组装器（稳定区有工具面和 system，示范对话随人格那一步）、事实模板、每件
    /// 工具的访问类别和参数格式、内核替工具写的几句、几样开关。
    ///
    /// # Errors
    ///
    /// 随核心附带的哪一份字用不了，写明是哪一类、哪里坏了；工具面上有两件同名的。
    pub fn policy(&self) -> Result<Policy, BuildError> {
        let core = &self.core;
        let ended = &core.turn_ended;
        let texts = Texts {
            checkpoint_open: core.checkpoint_open.clone(),
            checkpoint_close: core.checkpoint_close.clone(),
            checkpoint_end: core.checkpoint_end.clone(),
            restored: self.rebuild_texts().map(RebuildTexts::wrap).transpose()?,
            turn_ended: miyu_assemble::TurnEndedTexts {
                interrupted: ended.interrupted.clone(),
                error: ended.error.clone(),
                step_limit: ended.step_limit.clone(),
                aborted: ended.aborted.clone(),
                restarted: ended.restarted.clone(),
            },
            summarize_task: core
                .compaction
                .as_ref()
                .map(|compaction| compaction.summarize_task.clone())
                .unwrap_or_default(),
            truncated: self
                .shorten_texts()
                .map(|shorten| shorten.truncated.clone())
                .unwrap_or_default(),
            summarize_system: self.summarize_system().unwrap_or_default().to_string(),
            summarize_instructions: core
                .compaction
                .as_ref()
                .map(|compaction| compaction.summarize_instructions.clone())
                .unwrap_or_default(),
            summarize_end: core
                .compaction
                .as_ref()
                .map(|compaction| compaction.summarize_end.clone())
                .unwrap_or_default(),
            jobs: core.jobs.as_ref().map(JobTexts::rendered).transpose()?,
            harness: core
                .harness
                .as_ref()
                .map(HarnessTexts::rendered)
                .transpose()?,
            peers: self.peer_texts()?,
            recap: self.recap(),
            title: self.title(),
            vision: self.vision(),
            forward: self.forward()?,
            private_media: self.private_media(),
            group: self.group.as_ref().map(GroupChat::texts).transpose()?,
        };
        let (mut face, mut rules) = tools::split(&self.tools)?;
        let loading =
            crate::loading::stub(&self.tools, core.loading.as_ref(), &mut face, &mut rules)
                .map_err(|error| BuildError::Texts {
                    which: "loading texts",
                    error,
                })?;
        let stable = Stable {
            tools: face,
            system: self.system.clone(),
            demos: self.demos.iter().flat_map(Demo::messages).collect(),
        };
        let facts = core.facts.templates()?.with_reminder(self.reminder.clone());
        // 裸的回合开始不注入事实（施工 D-1 上）：请求和原来的一次性请求逐字节一样。
        let facts = if self.bare { facts.quiet() } else { facts };
        Ok(Policy {
            assembler: Box::new(DefaultAssembler::new(stable, texts)),
            facts,
            tools: rules,
            step_limit: self.step_limit,
            tool_texts: self.tool_texts()?,
            attended: self.attended,
            resumes: self.resumes,
            compaction: self.compaction(),
            notes: self.notes()?,
            reports: self.reports()?,
            titles: self.titles(),
            peers: self.peers(),
            nested: self.nested_limits(),
            foreground: self.foreground,
            loading,
        })
    }

    /// 检查点里代码写的几段的模板：压后重建的几段（施工 6-5），有截短重试的字的，加上摘要没看到的那一段（施工 6-6 中）。
    fn notes(&self) -> Result<Option<Notes>, BuildError> {
        let Some(mut notes) = self.rebuild_texts().map(RebuildTexts::notes).transpose()? else {
            return Ok(None);
        };
        notes.uncovered = self
            .shorten_texts()
            .map(ShortenTexts::uncovered)
            .transpose()?;
        Ok(Some(notes))
    }

    /// 隔离式那一句 system：有的才改走隔离式（施工 6-6 下）。
    fn summarize_system(&self) -> Option<&str> {
        self.core.compaction.as_ref()?.summarize_system.as_deref()
    }

    /// 截短重试的字：有的才截短（施工 6-6 中）。
    fn shorten_texts(&self) -> Option<&ShortenTexts> {
        self.core.compaction.as_ref()?.shorten.as_ref()
    }

    /// 压后重建的字：有的才写那几段、重读（施工 6-5）。
    fn rebuild_texts(&self) -> Option<&RebuildTexts> {
        self.core.compaction.as_ref()?.rebuild.as_ref()
    }

    /// 压缩的数和摘要指令都有，才主动压。
    fn compaction(&self) -> Option<Compaction> {
        let numbers = self.compaction?;
        self.core.compaction.as_ref()?;
        Some(Compaction {
            reserve_cap: numbers.reserve_cap,
            margin: numbers.margin,
            line_percent: numbers.line_percent,
            margin_percent: numbers.margin_percent,
            tail: numbers.tail,
            lead: numbers.lead,
            price: Flat {
                image: numbers.image,
                file: numbers.file,
            },
            rebuild: numbers
                .rebuild
                .filter(|_| self.rebuild_texts().is_some())
                .map(RebuildNumbers::kernel),
            pause: numbers.pause.map(PauseNumbers::kernel),
            shorten: numbers
                .shorten
                .filter(|_| self.shorten_texts().is_some())
                .map(ShortenNumbers::kernel),
            isolate: self.summarize_system().is_some(),
        })
    }

    /// 驱动的占位：图片、文件发不了，工具一个字都没回，附件挪到了后面，文本文件照字放进消息，带名字的图片。
    ///
    /// # Errors
    ///
    /// 占位的模板坏了。
    pub fn driver_texts(&self) -> Result<DriverTexts, BuildError> {
        self.core
            .drivers
            .texts()
            .map_err(|error| BuildError::Texts {
                which: "driver placeholders",
                error,
            })
    }

    /// 内核替工具写的几句。
    fn tool_texts(&self) -> Result<ToolTexts, BuildError> {
        let results = &self.core.tool_results;
        ToolTexts::new(ToolTextSources {
            unknown: &results.unknown,
            not_an_object: &results.not_an_object,
            cancelled_before: &results.cancelled_before,
            cancelled_running: &results.cancelled_running,
            skipped: &results.skipped,
            read_only: &results.read_only,
            denied: &results.denied,
            denied_with_reason: &results.denied_with_reason,
            unattended: &results.unattended,
            question_interrupted: &results.question_interrupted,
            question_voided: &results.question_voided,
            question_unattended: &results.question_unattended,
            restarted: &results.restarted,
        })
        .map_err(|error| BuildError::Texts {
            which: "kernel's tool result texts",
            error,
        })
    }
}

mod compaction;
mod error;
mod texts;
pub use compaction::{CompactionNumbers, LEAD, LINE_PERCENT, MARGIN_PERCENT, TAIL};
pub use error::{BuildError, SnapshotError};
pub use texts::{CompactionTexts, PermissionTexts, ToolResultTexts, TurnEndedTexts};

#[cfg(test)]
mod tests;
