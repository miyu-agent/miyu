//! 会话这一块的配置项（`docs/blueprint/compaction.md` 第十五条第 7 条，施工 6-11 上）：提前压好 `compaction.prepare`。
//!
//! 会话 actor 在回合开始冻结这一轮的配置时读它（`actor/model.rs` 的 `turn_start`），跟着 `TurnStartHooksDone` 交给内核：
//! 下一轮生效。不开回合的手动压缩照上一轮的。
//!
//! 抽取的三项（施工 R-6 上，`memory.md` 第六条）：闲多久、攒几轮、照哪个模型。会话 actor 从忙到闲起闹钟时照这一轮的配置读。
//! 合并的两项（施工 R-7 上，第七条）：多久合一次、攒几个会话；抽取记下以后照那一轮的配置读。
//! 「AI 怎么认识你」的两项（施工 P-9 端点声明，R-6 再修挪到这里，同一份）：造会话时端点照它拼「关于你」；抽取、合并照这一轮的
//! 配置读称呼，写人。
//! 核心替人格记忆这个内置包声明（施工 F-4，设计 `30-插件框架.md` 第七节）：挂在「软件包」那一页、`memory` 那一组；包没装的照样
//! 认，设置页不画（`miyu-core` 的 `settings::Packaged`）。

use std::time::Duration;

miyu_config::settings! {
    /// 压缩的配置。
    pub struct CompactionSettings in "compaction" {
        /// 提前压好：用量快到压缩线时在后台先把旧的那一段压好，到线直接换上，不用停下来等摘要。关了照到线再压。
        prepare: bool = true {
            kind: bool,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "general", group: "compaction", control: toggle },
        },
    }
}

miyu_config::settings! {
    /// 沙盒的配置（施工 5-16 下，`designs/11-权限与沙盒.md` 第四节「设置页「隐私与安全」」）：回合开始照这一轮的配置读，下一轮生效。
    pub struct SandboxSettings in "sandbox" {
        /// 沙盒强度：`auto` 有能用的 bubblewrap 就用它（能写的范围里的私密目录也藏），`weak` 只用 Landlock。只在 Linux 上起作用：
        /// 三个系统都登记（生成的文件哪个系统都一样），别的系统上 `config.schema` 标成不画。
        strength: String = "auto" {
            kind: option ["auto", "weak"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "privacy", group: "sandbox", control: select },
        },
    }
}

/// 沙盒强度设成只用 Landlock 的那一档（施工 5-16 下）。
pub const WEAK: &str = "weak";

miyu_config::settings! {
    /// 后台命令的配置（施工 7-12 下）：派活时照这一轮的配置读，下一轮生效。
    pub struct JobSettings in "jobs" {
        /// 后台命令最多跑多久，到了整组停掉（照 Claude Code：默认 30 分钟，最多 2 小时）。
        max_running: Duration = "30m" {
            kind: duration [60, 7200],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "general", group: "sessions", control: text },
        },
    }
}

miyu_config::settings! {
    /// 记忆抽取、合并的配置（施工 R-6 上、R-7 上）。
    pub struct MemorySettings in "memory" {
        /// 会话闲了多久才抽新的一段：短于常见的缓存寿命。
        extract_idle: Duration = "5m" {
            kind: duration [60, 3600],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", package: "memory", control: text },
        },
        /// 上次抽到以后至少有几轮她答了话才抽。
        extract_turns: i64 = 2 {
            kind: int [1, 100],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", package: "memory", control: number },
        },
        /// 整理记忆用的模型：模型或者 `@池`；没写的照 `models.chat`。
        organizer: Option<String> = none {
            kind: reference,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", package: "memory", control: text },
        },
        /// 离上次合并至少多久才再合（施工 R-7 上）。
        merge_every: Duration = "24h" {
            kind: duration [3600, 2592000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", package: "memory", control: text },
        },
        /// 上次合并以后至少抽过几个会话才再合（施工 R-7 上）。
        merge_sessions: i64 = 5 {
            kind: int [1, 100],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", package: "memory", control: number },
        },
        /// 每一间留最近几趟整理会话（施工 D-1 下，设计 41 DS9）：一趟跑完删多的，`0` 是跑完就删。
        dream_keep: i64 = 20 {
            kind: int [0, 1000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", package: "memory", control: number },
        },
    }
}

miyu_config::settings! {
    /// 「AI 怎么认识你」（施工 P-9，2026-10-13 项目主人定：两项，称呼和更多描述；P-9 补 2026-10-16 加回复语言，2026-10-17 项目主人定排第一；和称呼一样标常用，头把常用项排在组里前面）：只写在个人设置里，新会话生效；设置页单独一页「用户」（P-9 再补，2026-10-16 项目主人定）。造会话时
    /// 照模板拼进 system（`miyu_policy::Profile`）；本机开的会话带，场所会话照桥交的 `profile`。抽取、合并照它写人（施工 R-6 再修，
    /// 从端点挪到这里：会话这一层要读）。
    pub struct ProfileSettings in "profile" {
        /// 希望她默认用哪种语言回（施工 P-9 补，2026-10-16 项目主人定）：可以空；设置页是下拉加「其他」手写，下拉里的建议值
        /// 在字表里（`suggestions`）。
        preferred_reply_language: Option<String> = none {
            kind: text [64],
            layers: [Personal],
            applies: new_session,
            ui: { page: "user", group: "profile", common: true, control: combo },
        },
        /// AI 怎么称呼你。
        name: Option<String> = none {
            kind: text [64],
            layers: [Personal],
            applies: new_session,
            ui: { page: "user", group: "profile", common: true, control: text },
        },
        /// 更多描述：可以空。
        about: Option<String> = none {
            kind: lines [2000],
            layers: [Personal],
            applies: new_session,
            ui: { page: "user", group: "profile", control: textarea },
        },
    }
}

#[cfg(test)]
mod tests;
