//! `miyu config set`、`unset`、`edit`、`trust` 给人看的字（施工 8-3，`docs/blueprint/config.md`「给人看的字」的「命令行」
//! 那张表、「样子」；CL-2 下照设计 47 第三节改：结果行动词打头、不带句号，报错交 [`Problem`]，建议另起一行）。值由调用的
//! 一方写成 TOML 交进来（`"zh"`、`true`）。

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// 一层在句子里的叫法：`system`、`project`、`env`，别的是默认值。英文带冠词。
    fn layer_in(&self, layer: &str) -> &'static str {
        match (self, layer) {
            (Language::Chinese, "system") => "系统配置",
            (Language::Chinese, "project") => "项目配置",
            (Language::Chinese, "env") => "环境变量",
            (Language::Chinese, _) => "默认值",
            (Language::English, "system") => "the system config",
            (Language::English, "project") => "the project config",
            (Language::English, "env") => "the environment",
            (Language::English, _) => "the default",
        }
    }

    /// `set` 成了：`键 = 值`，什么时候生效（认不出的不说）。只写系统配置，不说写进了哪一层。
    pub(crate) fn saved(&self, key: &str, value: &str, applies: &str) -> String {
        match (self, self.applies_after(applies)) {
            (Language::Chinese, Some(applies)) => format!("已设置 {key} = {value}（{applies}）"),
            (Language::Chinese, None) => format!("已设置 {key} = {value}"),
            (Language::English, Some(applies)) => format!("set {key} = {value} ({applies})"),
            (Language::English, None) => format!("set {key} = {value}"),
        }
    }

    /// `set` 成了，可上面一层 `above` 压着、用的还是那一层写的 `effective`：结果行之后的告警（设计 47 第四节）。
    pub(crate) fn overridden(&self, key: &str, above: &str, effective: &str) -> Problem {
        let says = self.layer_in(above);
        match self {
            Language::Chinese => Problem::new(format!("{key} 被{says}覆盖，使用 {effective}")),
            Language::English => {
                Problem::new(format!("{key} is overridden by {says}; using {effective}"))
            }
        }
    }

    /// `set` 的值这一层本来就是。
    pub(crate) fn already(&self, value: &str) -> String {
        match self {
            Language::Chinese => format!("已经是 {value}，未修改"),
            Language::English => format!("already {value}, nothing changed"),
        }
    }

    /// `unset` 成了：删掉了哪一项，现在是哪一层的什么。
    pub(crate) fn removed(&self, key: &str, effective: &str, from: &str) -> String {
        let from = self.layer_name(from);
        match self {
            Language::Chinese => format!("已删除 {key}，现在是 {effective}（{from}）"),
            Language::English => format!("removed {key}, now {effective} ({from})"),
        }
    }

    /// `unset` 的这一层本来就没写。
    pub(crate) fn not_there(&self, key: &str, layer: &str) -> String {
        match self {
            Language::Chinese => format!("{}中没有 {key}，未修改", self.layer_in(layer)),
            Language::English => {
                format!("{key} is not in {}, nothing changed", self.layer_in(layer))
            }
        }
    }

    /// `set --project`：项目配置只能手改。
    pub(crate) fn project_by_hand(&self) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new("项目配置只能手动编辑").hint("运行 'miyu config edit --project'")
            }
            Language::English => Problem::new("the project config can only be edited by hand")
                .hint("run 'miyu config edit --project'"),
        }
    }

    /// `edit` 不在终端里。
    pub(crate) fn edit_needs_terminal(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("'miyu config edit' 需要在终端中运行"),
            Language::English => Problem::new("'miyu config edit' needs a terminal"),
        }
    }

    /// `edit` 没改。
    pub(crate) fn nothing_changed(&self) -> &'static str {
        match self {
            Language::Chinese => "未修改",
            Language::English => "nothing changed",
        }
    }

    /// `edit` 有错，问接着改还是放弃：印在标准错误上，不换行，后面等人敲。
    pub(crate) fn edit_errors(&self, errors: usize) -> String {
        match (self, errors) {
            (Language::Chinese, n) => {
                format!("有 {n} 处错误，未保存。按回车继续编辑，输入 q 放弃：")
            }
            (Language::English, 1) => {
                "1 error, not saved. Press Enter to keep editing, or type q to give up: "
                    .to_string()
            }
            (Language::English, n) => format!(
                "{n} errors, not saved. Press Enter to keep editing, or type q to give up: "
            ),
        }
    }

    /// `edit` 放弃了。
    pub(crate) fn gave_up(&self) -> &'static str {
        match self {
            Language::Chinese => "已放弃，文件未修改",
            Language::English => "gave up, file unchanged",
        }
    }

    /// `edit` 存的时候文件被别处改过了：改的留在副本 `copy` 里。
    pub(crate) fn edit_conflict(&self, copy: &str) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new("编辑期间文件已被修改，未保存").hint(format!("修改内容保留在 {copy}"))
            }
            Language::English => Problem::new("the file changed during editing; not saved")
                .hint(format!("the edited copy is kept in {copy}")),
        }
    }

    /// `edit` 的编辑器没有正常退出：退出码 `code`，被信号停掉的没有。
    pub(crate) fn editor_failed(&self, code: Option<i32>) -> Problem {
        let code = code.map_or_else(|| "-".to_string(), |code| code.to_string());
        match self {
            Language::Chinese => Problem::new(format!("编辑器异常退出（{code}），未保存")),
            Language::English => Problem::new(format!(
                "the editor did not exit cleanly ({code}); not saved"
            )),
        }
    }

    /// `edit` 存好了：改了的几项什么时候生效（照先后，一样的只说一次）；一项都没变的（只动了注释）不说，认不出的也不说。
    pub(crate) fn edit_saved(&self, applies: &[&str]) -> String {
        let said: Vec<String> = applies
            .iter()
            .filter_map(|applies| self.applies_after(applies))
            .collect();
        match (self, said.is_empty()) {
            (Language::Chinese, true) => "已保存".to_string(),
            (Language::English, true) => "saved".to_string(),
            (Language::Chinese, false) => format!("已保存（{}）", said.join("、")),
            (Language::English, false) => format!("saved ({})", said.join(", ")),
        }
    }

    /// `trust` 这里没有项目配置。
    pub(crate) fn no_project_here(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("当前目录没有项目配置"),
            Language::English => Problem::new("no project config here"),
        }
    }

    /// `trust` 列出它会改哪几项的第一行：项目配置只能让限制更严。
    pub(crate) fn would_set(&self, file: &str) -> String {
        match self {
            Language::Chinese => format!("{file} 会修改以下设置（项目配置只能收紧限制）："),
            Language::English => {
                format!("{file} would set (a project config can only tighten limits):")
            }
        }
    }

    /// `trust` 问信不信任 `file`：印在标准错误上，不换行，后面等人敲（设计 47 第三节第 6 条）。
    pub(crate) fn trust_question(&self, file: &str) -> String {
        match self {
            Language::Chinese => format!("信任 '{file}'？[y/N] "),
            Language::English => format!("Trust '{file}'? [y/N] "),
        }
    }

    /// `trust` 不在终端里、又没写 `--yes`、`--no`。
    pub(crate) fn trust_needs_terminal(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("需要在终端中确认").hint("加上 '--yes' 或 '--no'"),
            Language::English => {
                Problem::new("confirmation needs a terminal").hint("pass '--yes' or '--no'")
            }
        }
    }

    /// `trust` 本来就信任着这一份。
    pub(crate) fn already_trusted(&self) -> &'static str {
        match self {
            Language::Chinese => "已经信任，未修改",
            Language::English => "already trusted, nothing changed",
        }
    }

    /// `trust` 看的时候它又被改过了。
    pub(crate) fn trust_conflict(&self) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new("查看期间文件已被修改").hint("重新运行 'miyu config trust'")
            }
            Language::English => {
                Problem::new("the file changed during review").hint("run 'miyu config trust' again")
            }
        }
    }

    /// `trust` 记下了：信任、不信任。
    pub(crate) fn trust_answered(&self, trusted: bool) -> &'static str {
        match (self, trusted) {
            (Language::Chinese, true) => "已信任（文件变化后会再次询问）",
            (Language::Chinese, false) => "已拒绝，不会使用（文件变化后会再次询问）",
            (Language::English, true) => "trusted (will ask again if the file changes)",
            (Language::English, false) => {
                "not trusted, will not be used (will ask again if the file changes)"
            }
        }
    }
}
