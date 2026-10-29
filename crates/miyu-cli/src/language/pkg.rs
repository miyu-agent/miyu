//! `miyu pkg` 给人看的字（施工 T-3，`docs/blueprint/cli/pkg.md`「给人看的字」；CL-2 补照设计 47 第三节改：结果行动词打头，
//! 报错交 [`Problem`]，由 `diagnostic` 加 `miyu: `，建议另起一行）。

mod info;
mod install;
mod pack;
mod source;

pub(crate) use info::{CameFrom, InfoKey};
pub(crate) use pack::LogoWhy;

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// `install` 装好了 `what`（编号，有版本的接版本）。
    pub fn installed(&self, what: &str) -> String {
        match self {
            Language::Chinese => format!("已安装 {what}"),
            Language::English => format!("installed {what}"),
        }
    }

    /// 装好以后问的那一句，后面不换行（作者说明第九节第 1 条）。
    pub(crate) fn open_now(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("现在启用 '{id}'？[Y/n] "),
            Language::English => format!("Enable '{id}' now? [Y/n] "),
        }
    }

    /// 启用了 `id`。
    pub(crate) fn opened(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("已启用 {id}"),
            Language::English => format!("enabled {id}"),
        }
    }

    /// 没启用：一行结果，一行建议（以后在「软件包」页里启用）。
    pub(crate) fn not_opened(&self, id: &str) -> [String; 2] {
        match self {
            Language::Chinese => [
                format!("未启用 {id}"),
                "提示：之后可以在界面的软件包页面启用".to_string(),
            ],
            Language::English => [
                format!("left {id} disabled"),
                "hint: enable it later on the Packages page".to_string(),
            ],
        }
    }

    /// `remove` 卸掉了 `id`。
    pub fn uninstalled(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("已卸载 {id}"),
            Language::English => format!("removed {id}"),
        }
    }

    /// 清单装不上：装的是 `what`（照写的原样，不知道的没有），哪里不对，知道第几行的照 `package.toml:<行>` 带上。
    pub(crate) fn cannot_install(
        &self,
        what: Option<&str>,
        problem: &str,
        line: Option<u64>,
    ) -> Problem {
        let problem = match line {
            Some(line) => format!("package.toml:{line}: {problem}"),
            None => problem.to_string(),
        };
        Problem::new(format!("{}{problem}", self.cannot_install_head(what)))
    }

    /// 「无法安装 `what`：」这一截。
    fn cannot_install_head(&self, what: Option<&str>) -> String {
        match (self, what) {
            (Language::Chinese, Some(what)) => format!("无法安装 {what}："),
            (Language::Chinese, None) => "无法安装：".to_string(),
            (Language::English, Some(what)) => format!("cannot install {what}: "),
            (Language::English, None) => "cannot install: ".to_string(),
        }
    }

    /// 和装着的包冲突（施工 F-8 六补，照 pacman 的「冲突的文件」）：一处一条报错，「无法安装 `what`：哪一样 名字 已在
    /// 哪个包 中」；核心没列是哪几处的说名称已被占用。`conflicts` 是核心交回的 `data.conflicts`。
    pub(crate) fn conflicting(
        &self,
        what: Option<&str>,
        conflicts: &[serde_json::Value],
    ) -> Vec<Problem> {
        let head = self.cannot_install_head(what);
        if conflicts.is_empty() {
            let said = match self {
                Language::Chinese => "名称已被占用",
                Language::English => "names already in use",
            };
            return vec![Problem::new(format!("{head}{said}"))];
        }
        conflicts
            .iter()
            .map(|one| {
                let (kind, name) = (
                    self.conflict_kind(one["kind"].as_str().unwrap_or_default()),
                    one["name"].as_str().unwrap_or_default(),
                );
                // 撞了人格、预设的一层的（施工 P-11 下）写 `layer`，不写 `package`。
                let holder = match one["layer"].as_str() {
                    Some(layer) => self.conflict_layer(layer).to_string(),
                    None => one["package"].as_str().unwrap_or_default().to_string(),
                };
                // 核心自带的斜杠命令（施工 BS-1 下下）。
                let core = one["core"].as_bool() == Some(true);
                let said = match (self, core) {
                    (Language::Chinese, true) => format!("{kind} {name} 是核心自带的"),
                    (Language::English, true) => format!("{kind} {name} is built into the core"),
                    (Language::Chinese, false) => format!("{kind} {name} 已在 {holder} 中"),
                    (Language::English, false) => format!("{kind} {name} exists in {holder}"),
                };
                Problem::new(format!("{head}{said}"))
            })
            .collect()
    }

    /// 冲突的那一样叫什么；不认识的照原样。
    fn conflict_kind<'a>(&self, kind: &'a str) -> &'a str {
        match (self, kind) {
            (Language::Chinese, "command") => "子命令",
            (Language::Chinese, "feature") => "功能",
            (Language::Chinese, "tool") => "工具",
            (Language::Chinese, "skill") => "技能",
            (Language::Chinese, "mcp") => "MCP 服务",
            (Language::Chinese, "knowledge") => "知识库",
            (Language::Chinese, "slash") => "斜杠命令",
            (Language::Chinese, "persona") => "人格",
            (Language::Chinese, "preset") => "预设",
            (Language::Chinese, "prefix") => "前缀",
            (Language::English, "mcp") => "MCP server",
            (Language::English, "knowledge") => "knowledge base",
            (Language::English, "slash") => "slash command",
            _ => kind,
        }
    }

    /// 人格、预设撞上的那一层叫什么（施工 P-11 下）；不认识的照原样。
    fn conflict_layer<'a>(&self, layer: &'a str) -> &'a str {
        match (self, layer) {
            (Language::Chinese, "shipped") => "出厂内容",
            (Language::Chinese, "home") => "数据根",
            (Language::English, "shipped") => "the shipped content",
            (Language::English, "home") => "the data root",
            _ => layer,
        }
    }

    /// 卸掉了的出厂的包接在名字后面的那一截。
    pub(crate) fn removed_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "（已卸载）",
            Language::English => " (removed)",
        }
    }

    /// 写错的包接在后面的那一截：哪里不对。
    pub(crate) fn broken_mark(&self, problem: &str) -> String {
        match self {
            Language::Chinese => format!("（清单有误：{problem}）"),
            Language::English => format!(" (invalid: {problem})"),
        }
    }

    /// `info` 的大小：多大、几个文件。
    pub(crate) fn package_size(&self, size: &str, files: u64) -> String {
        match self {
            Language::Chinese => format!("{size}，{files} 个文件"),
            Language::English if files == 1 => format!("{size}, 1 file"),
            Language::English => format!("{size}, {files} files"),
        }
    }

    /// `info` 的安装时间那一格，出厂的包写的。
    pub(crate) fn shipped(&self) -> &'static str {
        match self {
            Language::Chinese => "出厂自带",
            Language::English => "shipped with Miyu",
        }
    }

    /// `owns`：`path` 是包 `id` 装的时候带的。
    pub(crate) fn owned_by(&self, path: &str, id: &str) -> String {
        match self {
            Language::Chinese => format!("{path} 属于 {id}"),
            Language::English => format!("{path} is owned by {id}"),
        }
    }

    /// `owns`：`path` 在包 `id` 的目录里，装的时候没有它（包自己后来写的）。
    pub(crate) fn inside_of(&self, path: &str, id: &str) -> String {
        match self {
            Language::Chinese => format!("{path} 在 {id} 的目录里，不是安装时带的"),
            Language::English => {
                format!("{path} is in the directory of {id} but was not installed with it")
            }
        }
    }

    /// `owns`：哪个包都没有 `path`。
    pub(crate) fn owned_by_none(&self, path: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("没有软件包包含 {path}")),
            Language::English => Problem::new(format!("no package owns {path}")),
        }
    }

    /// `check` 的一行：包 `id` 的文件 `path` 怎么了（`modified`、`missing`、`extra`），都没问题的 `path` 是空的。
    pub(crate) fn checked(&self, id: &str, what: Checked, path: &str) -> String {
        let word = match (self, what) {
            (Language::Chinese, Checked::Modified) => "已修改",
            (Language::Chinese, Checked::Missing) => "缺失",
            (Language::Chinese, Checked::Extra) => "多出",
            (Language::Chinese, Checked::Fine) => return format!("{id}：正常"),
            (Language::English, Checked::Modified) => "modified",
            (Language::English, Checked::Missing) => "missing",
            (Language::English, Checked::Extra) => "extra",
            (Language::English, Checked::Fine) => return format!("{id}: OK"),
        };
        match self {
            Language::Chinese => format!("{id}：{word} {path}"),
            Language::English => format!("{id}: {word} {path}"),
        }
    }

    /// `check`：家目录里没有记了文件的包。
    pub(crate) fn nothing_to_check(&self) -> &'static str {
        match self {
            Language::Chinese => "没有可检查的软件包",
            Language::English => "no packages to check",
        }
    }
}

/// `check` 查出来的一个文件怎么了；`Fine` 是整个包都没问题。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Checked {
    Modified,
    Missing,
    Extra,
    Fine,
}
