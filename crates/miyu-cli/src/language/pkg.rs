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
            // 核心自带的斜杠命令（施工 BS-1 下下）。
            let core = one["core"].as_bool() == Some(true);
            said.push(match (self, core) {
                (Language::Chinese, true) => format!("{kind} {name} 是核心自带的"),
                (Language::English, true) => format!("{kind} {name} is built into the core"),
                (Language::Chinese, false) => format!("{kind} {name} 已在 {holder} 中"),
                (Language::English, false) => format!("{kind} {name} exists in {holder}"),
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

/// `miyu pkg pack` 的字（施工 G-1）。
impl Language {
    /// 打好了，写在 `path`。
    pub(crate) fn packed(&self, path: &str) -> String {
        match self {
            Language::Chinese => format!("打好了：{path}"),
            Language::English => format!("Packed {path}"),
        }
    }

    /// 包文件多大。
    pub(crate) fn packed_size(&self, size: u64) -> String {
        match self {
            Language::Chinese => format!("大小：{size} 字节"),
            Language::English => format!("Size: {size} bytes"),
        }
    }

    /// 包的图标超了上限，打进去的是缩好的那一份（施工 F-18 上）：源文件夹不动。
    pub(crate) fn logo_shrunk(
        &self,
        logo: &str,
        (width, height, size): (u32, u32, usize),
    ) -> String {
        match self {
            Language::Chinese => {
                format!(
                    "图标 {logo} 超了上限，打进去的缩成 {width}×{height}、{size} 字节（源文件没动）"
                )
            }
            Language::English => format!(
                "Logo {logo} was over the limit; packed a {width}×{height}, {size}-byte copy (the source file is unchanged)"
            ),
        }
    }

    /// 包的图标打不进去（施工 F-18 上）：不在、不是 PNG / WebP、缩不到上限以内。
    pub(crate) fn bad_logo(&self, logo: &str, why: LogoWhy) -> String {
        match (self, why) {
            (Language::Chinese, LogoWhy::Missing) => format!("打包失败：包文件夹里没有图标 {logo}"),
            (Language::English, LogoWhy::Missing) => {
                format!("Cannot pack: the logo {logo} is not in the package folder")
            }
            (Language::Chinese, LogoWhy::NotImage) => {
                format!("打包失败：图标 {logo} 不是 PNG 或 WebP 图片")
            }
            (Language::English, LogoWhy::NotImage) => {
                format!("Cannot pack: the logo {logo} is not a PNG or WebP image")
            }
            (Language::Chinese, LogoWhy::TooBig) => {
                format!("打包失败：图标 {logo} 缩不到 256 KiB、512×512 以内，换一张简单些的图")
            }
            (Language::English, LogoWhy::TooBig) => format!(
                "Cannot pack: the logo {logo} can't be shrunk under 256 KiB and 512×512; use a simpler picture"
            ),
        }
    }

    /// 文件夹名不是包的编号。
    pub(crate) fn not_a_package_id(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("文件夹名不是包的编号：{name}"),
            Language::English => format!("The folder name is not a package id: {name}"),
        }
    }

    /// 读不了清单。
    pub(crate) fn unreadable(&self, path: &str) -> String {
        match self {
            Language::Chinese => format!("读不了：{path}"),
            Language::English => format!("Cannot read {path}"),
        }
    }

    /// 清单写错了、打不成。
    pub(crate) fn cannot_pack(&self, why: &str) -> String {
        match self {
            Language::Chinese => format!("打包失败：{why}"),
            Language::English => format!("Cannot pack: {why}"),
        }
    }

    /// 清单没写版本。
    pub(crate) fn no_version(&self) -> String {
        match self {
            Language::Chinese => "清单里没写版本（[package] version）".to_string(),
            Language::English => "The manifest has no version ([package] version)".to_string(),
        }
    }

    /// 清单没写 `targets`，却给了 `--target`。
    pub(crate) fn no_targets(&self) -> String {
        match self {
            Language::Chinese => "清单没写 targets，不用 --target".to_string(),
            Language::English => "The manifest has no targets: leave out --target".to_string(),
        }
    }

    /// 清单写了 `targets`，要用 `--target` 挑一种。
    pub(crate) fn needs_target(&self, listed: &str) -> String {
        match self {
            Language::Chinese => format!("这个包只能装在 {listed} 上，用 --target 挑一种"),
            Language::English => format!("This package is for {listed}: pick one with --target"),
        }
    }

    /// `--target` 不在清单的 `targets` 里。
    pub(crate) fn not_listed_target(&self, asked: &str, listed: &str) -> String {
        match self {
            Language::Chinese => format!("--target {asked} 不在清单的 targets 里（{listed}）"),
            Language::English => {
                format!("--target {asked} is not in the manifest's targets ({listed})")
            }
        }
    }
}

/// `miyu pkg update` 的字（施工 G-3 上）。
impl Language {
    /// 拿到了新的索引：几个包、索引的时刻。
    pub(crate) fn index_updated(&self, count: u64, generated: &str) -> String {
        match self {
            Language::Chinese => format!("软件源已更新：{count} 个包（{generated}）"),
            Language::English => format!("Package index updated: {count} packages ({generated})"),
        }
    }

    /// 没拿成，用的是本机那份。
    pub(crate) fn index_stale(&self, generated: &str, why: &str) -> String {
        match self {
            Language::Chinese => format!("软件源连不上，用的是本机的索引（{generated}）：{why}"),
            Language::English => {
                format!("Package source unreachable, using the local index ({generated}): {why}")
            }
        }
    }
}

/// `miyu pkg search` 的几个标记（施工 G-3 下）。
impl Language {
    /// `-S` 什么都没带：照 pacman 的原话（项目主人 2026-10-19 定）。
    pub(crate) fn nothing_to_sync(&self) -> &'static str {
        match self {
            Language::Chinese => "错误：没有指定目标（使用 -h 获取帮助）",
            Language::English => "error: no targets specified (use -h for help)",
        }
    }

    /// `clean`：清掉了几个包文件、腾出多少；一个都没有的说没有要清的。
    pub(crate) fn cleaned(&self, removed: u64, freed: &str) -> String {
        match (self, removed) {
            (Language::Chinese, 0) => "缓存里没有要清的包文件".to_string(),
            (Language::Chinese, _) => format!("清掉了 {removed} 个包文件，腾出 {freed}"),
            (Language::English, 0) => "No package files to clean".to_string(),
            (Language::English, _) => format!("Removed {removed} package files, freed {freed}"),
        }
    }

    /// 装着的。
    pub(crate) fn installed_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "（已装）",
            Language::English => " (installed)",
        }
    }

    /// 不是官方的：没人审过。
    pub(crate) fn community_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "（社区）",
            Language::English => " (community)",
        }
    }

    /// 被拉黑了。
    pub(crate) fn blocked_mark(&self, reason: &str) -> String {
        match self {
            Language::Chinese => format!("（被拉黑：{reason}）"),
            Language::English => format!(" (blocked: {reason})"),
        }
    }
}

/// `miyu pkg upgrade` 的字（施工 G-5）。
impl Language {
    /// 没有要升级的。
    pub(crate) fn nothing_to_upgrade(&self) -> &'static str {
        match self {
            Language::Chinese => "没有要升级的软件包",
            Language::English => "Nothing to upgrade",
        }
    }

    /// 升级好了一个。
    pub(crate) fn upgraded(&self, id: &str, version: &str) -> String {
        match self {
            Language::Chinese => format!("升级好了：{id} {version}"),
            Language::English => format!("Upgraded {id} to {version}"),
        }
    }

    /// 软件源里没有了：照旧用着。
    pub(crate) fn upgrade_gone(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("提示：{id} 在软件源里没有了，照旧用着装好的那一版"),
            Language::English => format!(
                "note: {id} is no longer in the package source; the installed version stays"
            ),
        }
    }

    /// 被拉黑了：照旧开着，卸不卸自己定。
    pub(crate) fn upgrade_blocked(&self, id: &str, reason: &str) -> String {
        match self {
            Language::Chinese => format!("提示：{id} 被拉黑了（{reason}），照旧开着，卸不卸你定"),
            Language::English => {
                format!("note: {id} was blocked ({reason}); it stays on, removing it is up to you")
            }
        }
    }

    /// 来源仓库变了：不升。
    pub(crate) fn upgrade_moved(&self, id: &str, now: &str) -> String {
        match self {
            Language::Chinese => format!("提示：{id} 在软件源里换成了 {now} 的，不升"),
            Language::English => {
                format!("note: {id} now comes from {now} in the package source; not upgraded")
            }
        }
    }

    /// `packages.hold` 里的：跳过。
    pub(crate) fn upgrade_held(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("提示：{id} 在不升级的包里，跳过"),
            Language::English => format!("note: {id} is held; skipped"),
        }
    }
}

/// 包的图标为什么打不进去（施工 F-18 上）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogoWhy {
    /// 包文件夹里没有。
    Missing,
    /// 不是 PNG、WebP。
    NotImage,
    /// 缩不到上限以内。
    TooBig,
}
