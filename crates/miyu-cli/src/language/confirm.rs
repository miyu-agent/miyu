//! `miyu pkg install`、`remove` 做之前印的那一份、问的那一句（施工 F-8 下补，`docs/blueprint/cli/pkg.md`「给人看的字」；CL-2 补
//! 照设计 47 第三节改：问的一句是「<动作> '<对象>'？[Y/n]」，没得答的是报错带建议）。

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// 装回、卸掉整个组（施工 BS-1 下中）时组里没有要动的包。
    pub(crate) fn nothing_in_group(&self, group: &str) -> String {
        match self {
            Language::Chinese => format!("{group} 中没有需要处理的软件包"),
            Language::English => format!("nothing to do in {group}"),
        }
    }

    /// 第一行：将安装、将装回、将卸载哪个包（带版本的接版本）。
    pub(crate) fn will(&self, doing: Doing, id: &str) -> String {
        match (self, doing) {
            (Language::Chinese, Doing::Install) => format!("将安装 {id}"),
            (Language::Chinese, Doing::Restore) => format!("将装回 {id}"),
            (Language::Chinese, Doing::Remove) => format!("将卸载 {id}"),
            (Language::English, Doing::Install) => format!("Will install {id}"),
            (Language::English, Doing::Restore) => format!("Will restore {id}"),
            (Language::English, Doing::Remove) => format!("Will remove {id}"),
            (Language::Chinese, Doing::Upgrade) => format!("将升级 {id}"),
            (Language::English, Doing::Upgrade) => format!("Will upgrade {id}"),
        }
    }

    /// 升级时接在第一行后面：换下的那一份的版本（没写版本的没有）。
    pub(crate) fn replacing(&self, version: Option<&str>) -> String {
        match (self, version) {
            (Language::Chinese, Some(version)) => format!("（替换 {version}）"),
            (Language::Chinese, None) => "（替换已装的）".to_string(),
            (Language::English, Some(version)) => format!(" (replaces {version})"),
            (Language::English, None) => " (replaces the installed one)".to_string(),
        }
    }

    /// 带的程序：`ui`、`process`、`worker`、`builtin`。
    pub(crate) fn program(&self, kind: &str) -> String {
        let word = match (self, kind) {
            (Language::Chinese, "ui") => "界面程序",
            (Language::Chinese, "process") => "扩展程序",
            (Language::Chinese, "worker") => "小程序",
            (Language::Chinese, "builtin") => "内置功能",
            (Language::English, "ui") => "interface",
            (Language::English, "process") => "extension",
            (Language::English, "worker") => "worker program",
            (Language::English, "builtin") => "built-in feature",
            (_, other) => other,
        };
        word.to_string()
    }

    /// 带的另几样：子命令、后台页、吉祥物、接的平台、几个配置项、MCP 服务（施工 X-2）、技能、脚本（施工 X-13 上）、
    /// 斜杠命令模板（施工 X-14 上补）。
    pub(crate) fn carried(&self, what: Carried<'_>) -> String {
        match (self, what) {
            (Language::Chinese, Carried::Command(name)) => format!("命令 miyu {name}"),
            (Language::Chinese, Carried::Page) => "后台页".to_string(),
            (Language::Chinese, Carried::Mascot) => "吉祥物".to_string(),
            (Language::Chinese, Carried::Theme) => "主题".to_string(),
            (Language::Chinese, Carried::Connection(platform)) => format!("接入 {platform}"),
            (Language::Chinese, Carried::Settings(n)) => format!("{n} 项设置"),
            (Language::Chinese, Carried::Mcp(name)) => format!("MCP 服务 {name}"),
            (Language::Chinese, Carried::Skill(name)) => format!("技能 {name}"),
            (Language::Chinese, Carried::Script(name)) => format!("脚本 {name}"),
            (Language::Chinese, Carried::Slash(name)) => format!("斜杠命令 /{name}"),
            (Language::Chinese, Carried::Persona(name)) => format!("人格 {name}"),
            (Language::Chinese, Carried::Preset(name)) => format!("预设 {name}"),
            (Language::English, Carried::Command(name)) => format!("command miyu {name}"),
            (Language::English, Carried::Page) => "admin page".to_string(),
            (Language::English, Carried::Mascot) => "mascot".to_string(),
            (Language::English, Carried::Theme) => "theme".to_string(),
            (Language::English, Carried::Connection(platform)) => format!("{platform} connection"),
            (Language::English, Carried::Settings(1)) => "1 setting".to_string(),
            (Language::English, Carried::Settings(n)) => format!("{n} settings"),
            (Language::English, Carried::Mcp(name)) => format!("MCP server {name}"),
            (Language::English, Carried::Skill(name)) => format!("skill {name}"),
            (Language::English, Carried::Script(name)) => format!("script {name}"),
            (Language::English, Carried::Slash(name)) => format!("slash command /{name}"),
            (Language::English, Carried::Persona(name)) => format!("persona {name}"),
            (Language::English, Carried::Preset(name)) => format!("preset {name}"),
        }
    }

    /// 读不成的一个：包文件夹里的路径和为什么（施工 X-13 补）。为什么是核心给的英文一句，同 `miyu check`。
    pub(crate) fn skipped(&self, file: &str, why: &str) -> String {
        match self {
            Language::Chinese => format!("{file}（{why}）"),
            Language::English => format!("{file} ({why})"),
        }
    }

    /// 一行的开头：包含、读不成、需要、一并删除、大小。
    pub(crate) fn plan_label(&self, label: PlanLabel) -> &'static str {
        match (self, label) {
            (Language::Chinese, PlanLabel::Includes) => "包含：",
            (Language::Chinese, PlanLabel::Skipped) => "无法读取：",
            (Language::Chinese, PlanLabel::Needs) => "需要：",
            (Language::Chinese, PlanLabel::AlsoDeletes) => "一并删除：",
            (Language::Chinese, PlanLabel::Size) => "大小：",
            (Language::Chinese, PlanLabel::With) => "一起装：",
            (Language::Chinese, PlanLabel::Download) => "下载：",
            (Language::Chinese, PlanLabel::From) => "来源：",
            (Language::English, PlanLabel::Includes) => "Includes: ",
            (Language::English, PlanLabel::Skipped) => "Unreadable: ",
            (Language::English, PlanLabel::Needs) => "Needs: ",
            (Language::English, PlanLabel::AlsoDeletes) => "Also deletes: ",
            (Language::English, PlanLabel::Size) => "Size: ",
            (Language::English, PlanLabel::With) => "Also installs: ",
            (Language::English, PlanLabel::Download) => "Download: ",
            (Language::English, PlanLabel::From) => "From: ",
        }
    }

    /// 一行里几样之间隔开的。
    pub(crate) fn list_separator(&self) -> &'static str {
        match self {
            Language::Chinese => "、",
            Language::English => ", ",
        }
    }

    /// 来源仓库后面接的：官方的、社区的（没人审过）。
    pub(crate) fn origin_mark(&self, official: bool) -> &'static str {
        match (self, official) {
            (Language::Chinese, true) => "（官方）",
            (Language::Chinese, false) => "（社区，未经审核）",
            (Language::English, true) => " (official)",
            (Language::English, false) => " (community, not reviewed)",
        }
    }

    /// 一并删除的配置项：写了的那几个键。
    pub(crate) fn deleted_settings(&self, keys: &str) -> String {
        match self {
            Language::Chinese => format!("设置 {keys}"),
            Language::English => format!("settings {keys}"),
        }
    }

    /// 一并删除的状态目录。
    pub(crate) fn state_dir(&self) -> &'static str {
        match self {
            Language::Chinese => "状态目录",
            Language::English => "state directory",
        }
    }

    /// 问的那一句，后面不换行：`what` 是包的编号或组名。
    pub(crate) fn proceed(&self, doing: Doing, what: &str) -> String {
        let action = match (self, doing) {
            (Language::Chinese, Doing::Install) => "安装",
            (Language::Chinese, Doing::Restore) => "装回",
            (Language::Chinese, Doing::Remove) => "卸载",
            (Language::Chinese, Doing::Upgrade) => "升级",
            (Language::English, Doing::Install) => "Install",
            (Language::English, Doing::Restore) => "Restore",
            (Language::English, Doing::Remove) => "Remove",
            (Language::English, Doing::Upgrade) => "Upgrade",
        };
        match self {
            Language::Chinese => format!("{action} '{what}'？[Y/n] "),
            Language::English => format!("{action} '{what}'? [Y/n] "),
        }
    }

    /// 升级不止一个时问的那一句，后面不换行。
    pub(crate) fn proceed_upgrades(&self, count: usize) -> String {
        match self {
            Language::Chinese => format!("升级 {count} 个软件包？[Y/n] "),
            Language::English => format!("Upgrade {count} packages? [Y/n] "),
        }
    }

    /// 答了不。
    pub(crate) fn cancelled(&self) -> &'static str {
        match self {
            Language::Chinese => "已取消",
            Language::English => "cancelled",
        }
    }

    /// 没得答（标准输入关着）：脚本里要写 `--yes`。
    pub(crate) fn unconfirmed(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("已取消：没有收到确认").hint("加 '--yes' 跳过确认"),
            Language::English => {
                Problem::new("cancelled: no confirmation").hint("pass '--yes' to skip the question")
            }
        }
    }
}

/// 做哪一件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Doing {
    Install,
    Restore,
    Remove,
    /// 升级（施工 G-5）。
    Upgrade,
}

/// 「包含」那一行除了程序的几样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Carried<'a> {
    Command(&'a str),
    Page,
    Mascot,
    /// 终端的主题（施工 F-16）。
    Theme,
    Connection(&'a str),
    Settings(u64),
    /// 带的 MCP 服务（施工 X-2）：装上以后会在这台机器上跑它的命令。
    Mcp(&'a str),
    /// 带的技能（施工 X-13 上）。
    Skill(&'a str),
    /// 带的脚本（施工 X-13 上）：装上以后是她的一件工具，在这台机器上跑。
    Script(&'a str),
    /// 带的斜杠命令模板（施工 X-14 上补）。
    Slash(&'a str),
    /// 带的人格（施工 P-11 下）。
    Persona(&'a str),
    /// 带的预设（施工 P-11 下）。
    Preset(&'a str),
}

/// 一行开头的词。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlanLabel {
    Includes,
    /// 读不成的技能、脚本（施工 X-13 补）。
    Skipped,
    Needs,
    AlsoDeletes,
    Size,
    /// 从软件源装时一起装的依赖（施工 G-4 下）。
    With,
    /// 从软件源装要下多大（施工 G-4 下）。
    Download,
    /// 从软件源装的来源仓库、官方还是社区（施工 G-4 下）。
    From,
}
