//! 会话这一块的配置项（`docs/blueprint/compaction.md` 第十五条第 7 条，施工 6-11 上）：提前压好 `compaction.prepare`；压缩
//! 触发线 `compaction.line`（施工 CT-1，第二条第 2 条）。
//!
//! 会话 actor 在回合开始冻结这一轮的配置时读它们（`actor/model.rs` 的 `turn_start`），跟着 `TurnStartHooksDone` 交给内核：
//! 下一轮生效。不开回合的手动压缩照上一轮的。
//!
//! 抽取的三项（施工 R-6 上，`memory.md` 第六条）：闲多久、攒几轮、照哪个模型。会话 actor 从忙到闲起闹钟时照这一轮的配置读。
//! 合并的两项（施工 R-7 上，第七条）：多久合一次、攒几个会话；抽取记下以后照那一轮的配置读。
//! 「AI 怎么认识你」的两项（施工 P-9 端点声明，R-6 再修挪到这里，同一份）：造会话时端点照它拼「关于你」；抽取、合并照这一轮的
//! 配置读称呼，写人。
//! 核心替人格记忆这个内置包声明（施工 F-4，设计 `30-插件框架.md` 第七节）：挂在「软件包」那一页、`base-memory` 那一组（组的编号是包编号，施工 PN-3）；包没装的照样
//! 认，设置页不画（`miyu-core` 的 `settings::Packaged`）。
//! 展示的一项（施工 D-5 补二）：system 里带不带 show 的那一句，同样核心替基础系统声明，造会话时读。
//! 受保护的命令的五项（施工 HZ-1，设计 `11-权限与沙盒.md` 的 A18）：完全放开下也先问的高危命令，一类一个开关。回合开始照这一
//! 轮的配置换进权限策略（`guard.rs`），下一轮生效。

use std::time::Duration;

use miyu_policy::hazard::Group;

miyu_config::settings! {
    /// 展示（施工 D-5 补二，项目主人 2026-10-24 定）：核心替包 `base-show` 声明，挂在「软件包」那一页、那个包的一组（施工 PN-3）。造会话时
    /// 读一次钉进快照（`miyu_policy::Snapshot::no_show_hint`），新会话生效：会话中途改了 system 不变，前缀稳定。
    pub struct ShowSettings in "show" {
        /// system 里带 show 的那一句（`show/system.txt`）：做出来、拿到的图、网页、文档、音视频用 `show` 交给人看。工具面上
        /// 有 `show` 的才带。
        system_hint: bool = true {
            kind: bool,
            layers: [System],
            applies: new_session,
            ui: { page: "packages", group: "base-show", package: "base-show", control: toggle },
        },
    }
}

miyu_config::settings! {
    /// 压缩的配置。
    pub struct CompactionSettings in "compaction" {
        /// 压缩触发线（施工 CT-1，2026-11-12 项目主人定）：上下文用到窗口的百分之几时自动压缩。压缩线照旧和「窗口 − 输出预留
        /// − 余量」取小的（`compaction.md` 第二条第 2 条）：20 万窗口调到 85 以上不起作用，百万窗口才到得了 95。回合开始照它换
        /// 策略快照里的数，会话钉了的照会话的。
        line: i64 = 85 {
            kind: int [20, 95],
            layers: [System],
            applies: next_turn,
            ui: { page: "advanced", group: "compaction", control: number },
        },
        /// 提前压好：用量快到压缩线时在后台先把旧的那一段压好，到线直接换上，不用停下来等摘要。关了照到线再压。
        prepare: bool = true {
            kind: bool,
            layers: [System],
            applies: next_turn,
            ui: { page: "advanced", group: "compaction", control: toggle },
        },
    }
}

miyu_config::settings! {
    /// 后台命令的配置（施工 7-12 下）：派活时照这一轮的配置读，下一轮生效。
    pub struct JobSettings in "jobs" {
        /// 后台命令最多跑多久，到了整组停掉（照 Claude Code：默认 30 分钟，最多 2 小时）。
        max_running: Duration = "30m" {
            kind: duration [60, 7200],
            layers: [System],
            applies: next_turn,
            ui: { page: "general", group: "sessions", control: text },
        },
    }
}

miyu_config::settings! {
    /// 记忆抽取、合并的配置（施工 R-6 上、R-7 上）。
    pub struct MemorySettings in "memory" {
        /// 会话闲了多久才抽新的一段：短于常见的缓存寿命。
        extract_idle: Duration = "30m" {
            kind: duration [60, 3600],
            layers: [System],
            applies: next_turn,
            ui: { page: "packages", group: "base-memory", package: "base-memory", control: text },
        },
        /// 上次抽到以后至少有几轮她答了话才抽。
        extract_turns: i64 = 2 {
            kind: int [1, 100],
            layers: [System],
            applies: next_turn,
            ui: { page: "packages", group: "base-memory", package: "base-memory", control: number },
        },
        /// 一直不闲的会话（施工 E-5）：上次抽以后结束了这么多轮就不等闲、在后台抽。
        extract_max_turns: i64 = 20 {
            kind: int [2, 500],
            layers: [System],
            applies: next_turn,
            ui: { page: "packages", group: "base-memory", package: "base-memory", control: number },
        },
        /// 一直不闲的会话（施工 E-5）：最早没抽的那一轮结束了这么久就不等闲、在后台抽。
        extract_max_wait: Duration = "1h" {
            kind: duration [600, 86400],
            layers: [System],
            applies: next_turn,
            ui: { page: "packages", group: "base-memory", package: "base-memory", control: text },
        },
        /// 离上次合并至少多久才再合（施工 R-7 上；施工 E-9 起后台整理只看它，不再看抽过几个会话）。
        merge_every: Duration = "16h" {
            kind: duration [3600, 2592000],
            layers: [System],
            applies: next_turn,
            ui: { page: "packages", group: "base-memory", package: "base-memory", control: text },
        },
        /// 每一间留最近几趟整理会话（施工 D-1 下，设计 41 DS9）：一趟跑完删多的，`0` 是跑完就删。
        dream_keep: i64 = 20 {
            kind: int [0, 1000],
            layers: [System],
            applies: next_turn,
            ui: { page: "packages", group: "base-memory", package: "base-memory", control: number },
        },
    }
}

miyu_config::settings! {
    /// 「AI 怎么认识你」（施工 P-9，2026-10-13 项目主人定：两项，称呼和更多描述；P-9 补 2026-10-16 加回复语言，2026-10-17 项目主人定排第一；和称呼一样标常用，头把常用项排在组里前面）：写在配置里，新会话生效；设置页单独一页「用户」（P-9 再补，2026-10-16 项目主人定）。造会话时
    /// 照模板拼进 system（`miyu_policy::Profile`）；本机开的会话带，场所会话照桥交的 `profile`。抽取、合并照它写人（施工 R-6 再修，
    /// 从端点挪到这里：会话这一层要读）。
    pub struct ProfileSettings in "profile" {
        /// 希望她默认用哪种语言回（施工 P-9 补，2026-10-16 项目主人定）：可以空；设置页是下拉加「其他」手写，下拉里的建议值
        /// 在字表里（`suggestions`）。
        preferred_reply_language: Option<String> = none {
            kind: text [64],
            layers: [System],
            applies: new_session,
            ui: { page: "user", group: "profile", common: true, control: combo },
        },
        /// AI 怎么称呼你。
        name: Option<String> = none {
            kind: text [64],
            layers: [System],
            applies: new_session,
            ui: { page: "user", group: "profile", common: true, control: text },
        },
        /// 更多描述：可以空。
        about: Option<String> = none {
            kind: lines [2000],
            layers: [System],
            applies: new_session,
            ui: { page: "user", group: "profile", control: textarea },
        },
    }
}

miyu_config::settings! {
    /// 受保护的命令（施工 HZ-1，设计 `11-权限与沙盒.md` 的 A18，2026-11-11 项目主人定）：完全放开下也先问人的高危命令，一类
    /// 一个开关，出厂都开；关掉的那一类在本人的会话里不再问、直接跑。外部身份的会话不看这几项；直接拒的那几种没有开关。设置页
    /// 「隐私与安全」那一页最上面一组。哪一种归哪一类照 [`miyu_policy::hazard::Group`]。
    pub struct ConfirmSettings in "permission.confirm" {
        /// 破坏性的 git 先问：强制推送、`reset --hard`、`clean -f`、丢掉改动、删 stash。
        git: bool = true {
            kind: bool,
            layers: [System],
            applies: next_turn,
            ui: { page: "privacy", group: "protected", control: toggle },
        },
        /// 递归删除先问：删工作目录或它的上级，删运行时才知道目标的整个目录。
        delete: bool = true {
            kind: bool,
            layers: [System],
            applies: next_turn,
            ui: { page: "privacy", group: "protected", control: toggle },
        },
        /// 关机、重启先问。
        power: bool = true {
            kind: bool,
            layers: [System],
            applies: next_turn,
            ui: { page: "privacy", group: "protected", control: toggle },
        },
        /// 下载下来直接执行（`curl … | sh`）先问。
        pipe_to_shell: bool = true {
            kind: bool,
            layers: [System],
            applies: next_turn,
            ui: { page: "privacy", group: "protected", control: toggle },
        },
        /// 容器要宿主的设备、进程或网络命名空间先问。
        container: bool = true {
            kind: bool,
            layers: [System],
            applies: next_turn,
            ui: { page: "privacy", group: "protected", control: toggle },
        },
    }
}

impl CompactionSettings {
    /// 这一轮该用的压缩触发线，窗口的百分之几（施工 CT-1）：会话钉的 `pinned`，没钉的照配置的。
    pub(crate) fn line_for(&self, pinned: Option<u64>) -> u64 {
        pinned.unwrap_or_else(|| u64::try_from(self.line).unwrap_or(miyu_policy::LINE_PERCENT))
    }

    /// `compaction.line` 能写的范围（施工 CT-1）：照这一项声明的，协议上钉会话的 `compaction_line` 照它查，不另抄一份。
    ///
    /// # Panics
    ///
    /// 实际不会：这一项照声明是整数。
    pub fn line_range() -> std::ops::RangeInclusive<u64> {
        let item = Self::ITEMS
            .iter()
            .find(|item| item.key == "compaction.line")
            .expect("声明里有这一项");
        match item.kind {
            miyu_config::Kind::Int { min, max } => {
                u64::try_from(min).unwrap_or(0)..=u64::try_from(max).unwrap_or(0)
            }
            _ => panic!("compaction.line 是整数"),
        }
    }
}

impl ConfirmSettings {
    /// 关掉的那几类（不再问的），写成 [`Group::code`]：权限策略拿它和工具报的高危归的类比（`miyu_tool::HazardAsk::class`）。
    pub(crate) fn waived(&self) -> Vec<&'static str> {
        [
            (Group::Git, self.git),
            (Group::Delete, self.delete),
            (Group::Power, self.power),
            (Group::PipeToShell, self.pipe_to_shell),
            (Group::Container, self.container),
        ]
        .into_iter()
        .filter(|(_, asks)| !asks)
        .map(|(group, _)| group.code())
        .collect()
    }
}

#[cfg(test)]
mod tests;
