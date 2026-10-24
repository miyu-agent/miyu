//! 拼快照（`docs/designs/26-提示词.md` 第四节「怎么拼」）：system 照第四节的先后排，每一块去掉末尾的
//! 空白，块和块之间空一行，没有的块不留空行。
//!
//! 施工 3-6（上）时只有人设。别的块跟着各自的功能来，按 J12 先实测证明不加不行：场所说明施工 7-5 加（子会话）；核心的
//! 几行施工 2-7 补加，权限那一行和本机文件的路径那一行，2026-10-01 主会话 A/B 实测过（`26-提示词.md` 第十节）。

use std::collections::BTreeMap;

use miyu_kernel::id::ContentHash;

use crate::pause::PAUSE;
use crate::persona::Demo;
use crate::preset::{PresetPin, ROLEPLAY};
use crate::rebuild::REBUILD;
use crate::recap::RECAP;
use crate::shorten::SHORTEN;
use crate::shown::Shown;
use crate::snapshot::{
    CompactionNumbers, CoreTexts, LEAD, LINE_PERCENT, MARGIN_PERCENT, Snapshot, TAIL,
};

/// 有计划的重启打断了一轮，再起来时连着接着干几次：`02-内核.md` 第六节「载入、崩溃、重启」的初值。
const RESUMES: u32 = 3;

/// 压缩用的数的出厂值（`compaction.md`「对外的样子」）：输出预留的上限 20000、余量 13000（照 Claude Code），压缩线至多
/// 窗口的 85%（2026-10-10 项目主人定）、余量至多窗口的 5%，一张图、一个文件各算 2000，尾巴至多 16000（2026-09-29 项目主人定），提前压好的提前量至多 16000（2026-10-07 项目主人
/// 定）；压后重建、熔断、截短重试照各自的出厂数。
const COMPACTION: CompactionNumbers = CompactionNumbers {
    reserve_cap: 20_000,
    margin: 13_000,
    line_percent: LINE_PERCENT,
    margin_percent: MARGIN_PERCENT,
    image: 2_000,
    file: 2_000,
    tail: TAIL,
    lead: LEAD,
    rebuild: Some(REBUILD),
    pause: Some(PAUSE),
    shorten: Some(SHORTEN),
};

/// 读好的原文：随核心附带的字，和这个人格的字。执行器从资源目录读（`miyu-store` 的资源目录）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    /// 随核心附带的字（`resources/core/`）。
    pub core: CoreTexts,
    /// 这个人格的字。
    pub persona: PersonaTexts,
    /// 角色扮演提示的包装（`core/facts/reminder-open.txt`、`reminder-close.txt`，施工 P-1 补）：拼进快照的 `reminder`，
    /// 不另存，没有角色扮演提示的快照字节不变。
    pub reminder: Wrap,
}

/// 包一段字的开头、收尾。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wrap {
    /// 开头。
    pub open: String,
    /// 收尾。
    pub close: String,
}

/// 一个人格的字（`<人格目录>/prompts/`，施工 P-1 上照几层叠好）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonaTexts {
    /// 人设（`persona.md`）：没有的是空的。
    pub persona: String,
    /// 示范对话（`examples.md`，`crate::persona::read_examples` 读好的）：没有的是空的。
    pub examples: Vec<Demo>,
    /// 角色扮演提示（`reminders.md`，施工 P-1 补）：整份是一条；没有的是空的。
    pub reminders: String,
}

/// 照 `26-提示词.md` 第四节拼出人格 `persona` 的快照。`attended` 是这个场所有没有人能确认。无人格的（施工 P-4 上）`sources`
/// 里人格的字是空的，system 里只有核心自己的那段，没有指纹：回合开始不看人格的文件。
pub fn compose(persona: Option<&str>, sources: Sources, attended: bool) -> Snapshot {
    let digest = persona.map(|_| sources.persona.digest());
    Snapshot {
        persona: persona.map(str::to_string),
        system: system(&[&sources.persona.persona]),
        demos: sources.persona.examples,
        tools: Vec::new(),
        core: sources.core,
        step_limit: None,
        attended,
        resumes: RESUMES,
        compaction: Some(COMPACTION),
        jobs: Some(crate::jobs::JOB_NUMBERS),
        recap: Some(RECAP),
        title: Some(crate::title::TITLE),
        peers: Some(crate::peers::PEERS),
        memory: None,
        reminder: reminder(&sources.persona.reminders, &sources.reminder),
        persona_digest: digest,
        preset: None,
        no_tools: false,
        group: None,
        foreground: false,
        no_show_hint: false,
        knowledge: None,
        skills: None,
        profile: None,
        project: None,
        appended: BTreeMap::new(),
        bare: false,
        plugin_tools: Vec::new(),
    }
}

impl PersonaTexts {
    /// 三份字的指纹（施工 P-1 再补）：拼进快照的 `persona_digest`，回合开始时执行器照它认出人格的文件改了。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：字和示范对话总写得成 JSON。
    pub fn digest(&self) -> ContentHash {
        let texts = (&self.persona, &self.examples, &self.reminders);
        ContentHash::of(&serde_json::to_vec(&texts).expect("字和示范对话写得成 JSON"))
    }
}

/// 角色扮演提示拼成的一块（施工 P-1 补）：开头、去掉末尾空白的原文、换行、收尾。原文不转义：是人格的作者写的。去掉末尾
/// 空白以后是空的，没有。
fn reminder(text: &str, wrap: &Wrap) -> Option<String> {
    let text = text.trim_end();
    (!text.is_empty()).then(|| format!("{}{text}\n{}", wrap.open, wrap.close))
}

/// 核心的几行（`26-提示词.md` 第四节第 3 块，施工 2-7 补）和风格锁（第 7 块，施工 P-1 补）：执行器从资源目录读好交进来，
/// 造会话时拼进 system。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreLines {
    /// `<permission>` 那一块怎么读、每一级能做什么、只有人能切（`core/permission-rule.txt`）。没有工具的会话不带。
    pub permission: String,
    /// 回答里提到本机的文件写绝对路径（`core/local-paths-rule.txt`）。
    pub local_paths: String,
    /// 本机是什么系统（`core/machine.txt`，施工 2-7 再补）：`{os}` 换成系统的名字。没有工具的会话不带。
    pub machine: String,
    /// 跑在哪个 harness 里、哪一版（`core/agent-version.txt`，施工 E-M，2026-10-22 项目主人定）：读的时候 `{version}` 已经换好。
    /// 有没有工具都带，接在本机是什么系统后面。
    pub harness: String,
    /// 风格锁（`core/style-lock.txt`）：只有带角色扮演提示的人格带（[`Snapshot::with_style_lock`]）。
    pub style_lock: String,
    /// 什么时候写成调工具的程序（`core/clicall.txt`，施工 M-4，设计 39 第九节）：工具面上有脚本里能调的工具才带
    /// （[`Snapshot::with_clicall`]）。
    pub clicall: String,
    /// 做出来、拿到的东西用 show 交给人看（show 包的 `system.txt`，施工 D-5 补二）：工具面上有 show 才带
    /// （[`Snapshot::with_show`]）。
    pub show: String,
}

impl Snapshot {
    /// 带上核心的几行（施工 2-7 补）：system 的第三块，接在人设、场所说明后面，所以在 [`Snapshot::with_tools`]、
    /// [`Snapshot::with_venue`] 以后最后调。块里一行一句，先权限、再本机文件的路径、最后本机是什么系统 `os`（施工 2-7 再补）；
    /// 工具面是空的会话用不上权限、系统那两句，不带（26 第十节）。以前造的快照 system 里没有这一块（施工 2-7 再补以前造的
    /// 没有系统那一句），载入照快照发，前缀一字不变。预设关了的那几行不带（施工 C-11，[`Shown`]）：权限那句跟着权限级别，
    /// 本机文件的路径跟着工作目录。
    #[must_use]
    pub fn with_core_lines(mut self, lines: &CoreLines, os: &str, shown: Shown) -> Snapshot {
        let tooled = !self.tools.is_empty();
        let machine = lines.machine.replace("{os}", os);
        let block = (tooled && shown.facts.permission)
            .then_some(lines.permission.as_str())
            .into_iter()
            .chain(shown.facts.workdir.then_some(lines.local_paths.as_str()))
            .chain((tooled && shown.machine).then_some(machine.as_str()))
            .chain(shown.harness.then_some(lines.harness.as_str()))
            .map(str::trim_end)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        self.system = system(&[&self.system, &block]);
        self
    }

    /// 带上什么时候写成调工具的程序的那一段（施工 M-4，设计 39 第九节）：`tells` 是工具面上有脚本里能调的工具（会话那一层照
    /// `shell` 在不在、`clicall`、访问类别算好交来）。接在核心的几行后面单成一段；没有的不带，system 一字不变。
    #[must_use]
    pub fn with_clicall(mut self, lines: &CoreLines, tells: bool) -> Snapshot {
        if tells {
            self.system = system(&[&self.system, lines.clicall.trim_end()]);
        }
        self
    }

    /// 带上 show 的那一句（施工 D-5 补二）：`shows` 是工具面上有 show（会话那一层看工具面交来）。接在调工具的程序那一段
    /// 后面单成一段，常量字节、前缀稳定；没有的不带，system 一字不变。
    #[must_use]
    pub fn with_show(mut self, lines: &CoreLines, shows: bool) -> Snapshot {
        if shows {
            self.system = system(&[&self.system, lines.show.trim_end()]);
        }
        self
    }

    /// 除了 system、示范对话、角色扮演提示和人格的指纹，别的格都一样（施工 P-1 再补）。回合开头照新的重拼的那一份，这几样以外
    /// 只有核心的字可能不一样：不一样的说明程序升级过。施工 P-1 三补起换快照时执行器的字一起换，它不再拦，只用来认「光是升级」：
    /// 人格、预设都没改、工具面也没变的不为它换。工具面、预设也不比（施工 P-2 下）：换预设时重新筛过。
    pub fn swappable(&self, new: &Snapshot) -> bool {
        let rest = |snapshot: &Snapshot| Snapshot {
            system: String::new(),
            demos: Vec::new(),
            tools: Vec::new(),
            reminder: None,
            persona_digest: None,
            preset: None,
            ..snapshot.clone()
        };
        rest(self) == rest(new)
    }

    /// 带上她用得上的技能那一块（施工 X-10 上，`skills.md`「技能」第 2 条）：system 第 4 块（26 第四节）的前一半，在
    /// [`Snapshot::with_core_lines`] 以后、[`Snapshot::with_knowledge`] 以前调；记进快照。没有的 system 一字不变。
    #[must_use]
    pub fn with_skills(mut self, block: Option<String>) -> Snapshot {
        if let Some(block) = &block {
            self.system = system(&[&self.system, block]);
        }
        self.skills = block;
        self
    }

    /// 带上她查得到的知识库那一块（施工 K-1 补，`knowledge.md`「列出来的那一块」）：system 第 4 块（26 第四节），在
    /// [`Snapshot::with_core_lines`] 以后、[`Snapshot::with_preset`] 以前调；记进快照，回合开始照它比。没有的 system 一字不变。
    #[must_use]
    pub fn with_knowledge(mut self, block: Option<String>) -> Snapshot {
        if let Some(block) = &block {
            self.system = system(&[&self.system, block]);
        }
        self.knowledge = block;
        self
    }

    /// 带上预设（施工 P-2 中）：记进快照；没开角色扮演的去掉角色扮演提示（人格的指纹照旧算原来的字，回合开始时不会当成改过）。
    /// system 一字不加：原来接在后面的「装了没开」那一行（Y8）2026-10-23 项目主人定整行删掉（施工 P-OFF），以前造的快照里
    /// 拼好的照旧发。
    #[must_use]
    pub fn with_preset(mut self, pin: Option<PresetPin>) -> Snapshot {
        let Some(pin) = pin else {
            return self;
        };
        if pin.off.iter().any(|software| software == ROLEPLAY) {
            self.reminder = None;
        }
        self.preset = Some(pin);
        self
    }

    /// 带上风格锁（施工 P-1 补）：system 的最后一块（26 第四节第 7 块），在 [`Snapshot::with_core_lines`] 以后调。只有带
    /// 角色扮演提示的人格带（2026-10-07 项目主人定），别的 system 一字不变。
    #[must_use]
    pub fn with_style_lock(mut self, lock: &str) -> Snapshot {
        if self.reminder.is_some() {
            self.system = system(&[&self.system, lock]);
        }
        self
    }

    /// 带上场所说明（施工 7-5）：system 的第二块，接在人设后面（26 第四节）。现在只有子会话有，原文是
    /// `core/jobs/subagent-venue.txt`（`agents.md` 第九条第 3 条）；照拼 system 的规矩去掉末尾的空白、空一行。
    #[must_use]
    pub fn with_venue(mut self, venue: &str) -> Snapshot {
        self.system = system(&[&self.system, venue]);
        self
    }
}

/// system：照先后，每一块去掉末尾的空白，没有的块不留空行，块和块之间空一行。
pub(crate) fn system(pieces: &[&str]) -> String {
    pieces
        .iter()
        .map(|piece| piece.trim_end())
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests;
