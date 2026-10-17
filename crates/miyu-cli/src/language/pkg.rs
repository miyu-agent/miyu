//! `miyu pkg` 给人看的字（施工 T-3，`docs/blueprint/cli/pkg.md`「给人看的字」）。

use super::Language;

impl Language {
    /// `install` 装好了 `id`。
    pub fn installed(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("装好了：{id}"),
            Language::English => format!("Installed {id}"),
        }
    }

    /// 装好以后问的那一句，后面不换行（作者说明第九节第 1 条）。
    pub(crate) fn open_now(&self) -> &'static str {
        match self {
            Language::Chinese => "现在打开？[Y/n] ",
            Language::English => "Turn it on now? [Y/n] ",
        }
    }

    /// 打开了 `id`。
    pub(crate) fn opened(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("打开了：{id}"),
            Language::English => format!("Turned on {id}"),
        }
    }

    /// 没打开：以后在「软件包」页里打开。
    pub(crate) fn not_opened(&self) -> &'static str {
        match self {
            Language::Chinese => "没有打开：以后在「软件包」页里打开",
            Language::English => "Not turned on: turn it on later on the Packages page",
        }
    }

    /// `remove` 卸掉了 `id`。
    pub fn uninstalled(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("卸掉了：{id}"),
            Language::English => format!("Removed {id}"),
        }
    }

    /// 清单装不上：哪里不对，知道第几行的带上。
    pub(crate) fn cannot_install(&self, problem: &str, line: Option<u64>) -> String {
        match (self, line) {
            (Language::Chinese, Some(line)) => format!("装不上：{problem}（第 {line} 行）"),
            (Language::Chinese, None) => format!("装不上：{problem}"),
            (Language::English, Some(line)) => format!("Cannot install: {problem} (line {line})"),
            (Language::English, None) => format!("Cannot install: {problem}"),
        }
    }

    /// 和装着的包冲突（施工 F-8 六补，照 pacman 的「冲突的文件」）：第一行说装不上，接着一处一行，「<哪一样> <名字> 已在
    /// <包> 中」。`conflicts` 是核心交回的 `data.conflicts`。
    pub(crate) fn conflicting(&self, conflicts: &[serde_json::Value]) -> String {
        let mut said = vec![match self {
            Language::Chinese => "装不上：名字已经有了".to_string(),
            Language::English => "Cannot install: names already in use".to_string(),
        }];
        for one in conflicts {
            let (kind, name) = (
                self.conflict_kind(one["kind"].as_str().unwrap_or_default()),
                one["name"].as_str().unwrap_or_default(),
            );
            // 撞了人格、预设的一层的（施工 P-11 下）写 `layer`，不写 `package`。
            let holder = match one["layer"].as_str() {
                Some(layer) => self.conflict_layer(layer).to_string(),
                None => one["package"].as_str().unwrap_or_default().to_string(),
            };
            said.push(match self {
                Language::Chinese => format!("{kind} {name} 已在 {holder} 中"),
                Language::English => format!("{kind} {name} exists in {holder}"),
            });
        }
        said.join("\n")
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
            (Language::English, "mcp") => "MCP server",
            (Language::English, "knowledge") => "knowledge base",
            (Language::English, "slash") => "slash command",
            _ => kind,
        }
    }

    /// 人格、预设撞上的那一层叫什么（施工 P-11 下）；不认识的照原样。
    fn conflict_layer<'a>(&self, layer: &'a str) -> &'a str {
        match (self, layer) {
            (Language::Chinese, "shipped") => "出厂的",
            (Language::Chinese, "system") => "系统区",
            (Language::Chinese, "home") => "你的家目录",
            (Language::English, "shipped") => "the shipped ones",
            (Language::English, "system") => "the system area",
            (Language::English, "home") => "your home",
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
            Language::Chinese => format!("（写错了：{problem}）"),
            Language::English => format!(" (broken: {problem})"),
        }
    }

    /// `info` 每一行前面的那个词：名称、版本、大小、安装时间（施工 F-8 下）。
    pub(crate) fn info_labels(&self) -> [&'static str; 4] {
        match self {
            Language::Chinese => ["名称", "版本", "大小", "安装时间"],
            Language::English => ["Name", "Version", "Size", "Installed"],
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
            Language::Chinese => format!("{path} 在 {id} 的目录里，安装时没有"),
            Language::English => {
                format!("{path} is in the directory of {id} but was not installed with it")
            }
        }
    }

    /// `owns`：哪个包都没有 `path`。
    pub(crate) fn owned_by_none(&self, path: &str) -> String {
        match self {
            Language::Chinese => format!("没有软件包包含 {path}"),
            Language::English => format!("No package owns {path}"),
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
            Language::English => "No packages to check",
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
