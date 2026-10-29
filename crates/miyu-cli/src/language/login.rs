//! `miyu provider login`、`list`、`logout` 给人看的字（施工 8-5，`docs/blueprint/config.md`「给人看的字」的「命令行」那张表、「样子」；
//! 图纸没写到的几句 2026-10-01 主会话照施工员的推荐定；CL-2 下照设计 47 第三节改：结果行动词打头、不带句号，报错交
//! [`Problem`]，建议另起一行）。key 从不出现在这里的任何一句里。

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// 交互着选的那张表的头一行：`login` 列出配置里用到的、设过的，`logout` 只列设过的。
    pub(crate) fn keys_heading(&self, logout: bool) -> &'static str {
        match (self, logout) {
            (Language::Chinese, false) => "配置中使用的密钥：",
            (Language::English, false) => "Keys used by the config:",
            (Language::Chinese, true) => "已设置的密钥：",
            (Language::English, true) => "Keys that are set:",
        }
    }

    /// 问选哪一个：`login` 还能敲一个新名字。
    pub(crate) fn pick_key(&self, logout: bool) -> &'static str {
        match (self, logout) {
            (Language::Chinese, false) => "选择编号，或输入新名字：",
            (Language::English, false) => "Pick a number, or type a new name: ",
            (Language::Chinese, true) => "选择编号：",
            (Language::English, true) => "Pick a number: ",
        }
    }

    /// 直接回车、读到头：没选。
    pub(crate) fn not_picked(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("未选择"),
            Language::English => Problem::new("nothing picked"),
        }
    }

    /// `login` 不写名字，一个能选的都没有（设计 47 第四节）。
    pub(crate) fn no_keys_used(&self) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new("没有使用密钥的供应商").hint("用 'miyu provider login <名字>' 添加")
            }
            Language::English => Problem::new("no provider uses a key")
                .hint("add one with 'miyu provider login <name>'"),
        }
    }

    /// 名字不合写法（写在命令行上的、敲的）。
    pub(crate) fn bad_key_name(&self, name: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("密钥名字无效：{name}"))
                .hint("以小写字母开头，只含小写字母、数字、- 和 _，最多 64 个字符"),
            Language::English => Problem::new(format!("invalid key name: {name}")).hint(
                "start with a lowercase letter, use only lowercase letters, digits, - and _, at most 64 characters",
            ),
        }
    }

    /// 不写名字又不在终端里：`command` 是 `provider login` 或 `provider logout`。
    pub(crate) fn pick_needs_terminal(&self, command: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new("需要在终端中选择")
                .hint(format!("或者指定名字：'miyu {command} <名字>'")),
            Language::English => Problem::new("picking needs a terminal")
                .hint(format!("or give a name: 'miyu {command} <name>'")),
        }
    }

    /// 问 key：不显示。
    pub(crate) fn paste_key(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("粘贴 {name} 的密钥（不显示）："),
            Language::English => format!("Paste the key for {name} (hidden): "),
        }
    }

    /// 读到的 key 去掉前后空白是空的。
    pub(crate) fn no_key_given(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("未收到密钥"),
            Language::English => Problem::new("no key given"),
        }
    }

    /// 贴 key 时取消了：按了 `Ctrl+C`，或者空行按了 `Ctrl+D`（施工 8-5 补）。
    pub(crate) fn key_paste_cancelled(&self) -> &'static str {
        match self {
            Language::Chinese => "已取消，未保存",
            Language::English => "cancelled, not saved",
        }
    }

    /// 存好了：`replaced` 的是换掉了原来的。
    pub(crate) fn key_saved(&self, name: &str, replaced: bool) -> String {
        match (self, replaced) {
            (Language::Chinese, false) => format!("已保存 {name} 的密钥"),
            (Language::Chinese, true) => format!("已替换 {name} 的密钥"),
            (Language::English, false) => format!("saved the key for {name}"),
            (Language::English, true) => format!("replaced the key for {name}"),
        }
    }

    /// 设没设。
    pub(crate) fn key_state(&self, set: bool) -> &'static str {
        match (self, set) {
            (Language::Chinese, true) => "已设置",
            (Language::Chinese, false) => "未设置",
            (Language::English, true) => "set",
            (Language::English, false) => "not set",
        }
    }

    /// 谁在用：几项连在一起。
    pub(crate) fn users(&self, used_by: &[&str]) -> String {
        let between = match self {
            Language::Chinese => "、",
            Language::English => ", ",
        };
        used_by.join(between)
    }

    /// `list` 一个都没设过。
    pub(crate) fn no_keys_yet(&self) -> &'static str {
        match self {
            Language::Chinese => "没有密钥",
            Language::English => "no keys",
        }
    }

    /// `logout` 选的时候一个都没设过。
    pub(crate) fn no_keys_set(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("没有已设置的密钥"),
            Language::English => Problem::new("no keys set"),
        }
    }

    /// 删掉了。
    pub(crate) fn key_deleted(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("已删除 {name} 的密钥"),
            Language::English => format!("deleted the key for {name}"),
        }
    }

    /// 要删的没设过。
    pub(crate) fn no_such_key(&self, name: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("{name} 没有设置密钥")),
            Language::English => Problem::new(format!("no key for {name}")),
        }
    }
}
