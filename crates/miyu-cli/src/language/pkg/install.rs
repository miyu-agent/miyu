//! `miyu pkg install` 从软件源装时分阶段印的字（施工 IN-3，设计 `49-装包照pacman.md` 第二节，`docs/blueprint/cli/pkg.md`「给人看的
//! 字」）：解析依赖、检查冲突、要装的和推荐的两行、来源、两句问、下载、核对、安装。

use super::super::Language;

impl Language {
    /// 看一眼之前：正在解析依赖。
    pub(crate) fn resolving(&self) -> &'static str {
        match self {
            Language::Chinese => "正在解析依赖…",
            Language::English => "resolving dependencies...",
        }
    }

    /// 看一眼之前：正在检查冲突。
    pub(crate) fn looking_for_conflicts(&self) -> &'static str {
        match self {
            Language::Chinese => "正在检查冲突…",
            Language::English => "looking for conflicting packages...",
        }
    }

    /// 要装的那一行开头：「软件包 (N)」。
    pub(crate) fn packages_count(&self, count: usize) -> String {
        match self {
            Language::Chinese => format!("软件包 ({count})"),
            Language::English => format!("Packages ({count})"),
        }
    }

    /// 推荐的那一行开头：「推荐 (N)」。
    pub(crate) fn recommended_count(&self, count: usize) -> String {
        match self {
            Language::Chinese => format!("推荐 ({count})"),
            Language::English => format!("Recommended ({count})"),
        }
    }

    /// 「来源」的值：官方的；别的源写源的名字；不是官方的另接没人审过。
    pub(crate) fn verified_source(&self, official: bool, source: &str) -> String {
        match (self, official) {
            (Language::Chinese, true) => "官方，签名已验证".to_string(),
            (Language::Chinese, false) => format!("{source}，签名已验证，没人审过"),
            (Language::English, true) => "official, signature verified".to_string(),
            (Language::English, false) => format!("{source}, signature verified, not reviewed"),
        }
    }

    /// 装不止一个时问的那一句，后面不换行。
    pub(crate) fn proceed_installs(&self, count: usize) -> String {
        match self {
            Language::Chinese => format!("安装 {count} 个软件包？[Y/n] "),
            Language::English => format!("Install {count} packages? [Y/n] "),
        }
    }

    /// 接着问推荐的那一句，后面不换行。
    pub(crate) fn proceed_recommended(&self, count: usize) -> String {
        match (self, count) {
            (Language::Chinese, _) => format!("同时安装推荐的 {count} 个软件包？[Y/n] "),
            (Language::English, 1) => "Also install 1 recommended package? [Y/n] ".to_string(),
            (Language::English, _) => format!("Also install {count} recommended packages? [Y/n] "),
        }
    }

    /// 下载那一段的开头。
    pub(crate) fn downloading(&self) -> &'static str {
        match self {
            Language::Chinese => "正在下载…",
            Language::English => "downloading packages...",
        }
    }

    /// 不在终端里：下完一个印一行，`name` 是「<编号>-<版本>」。
    pub(crate) fn downloaded(&self, name: &str, size: &str) -> String {
        match self {
            Language::Chinese => format!("已下载 {name} {size}"),
            Language::English => format!("downloaded {name} {size}"),
        }
    }

    /// 「(i/N) 正在核对完整性」。
    pub(crate) fn checking_integrity(&self, index: u64, count: u64) -> String {
        match self {
            Language::Chinese => format!("({index}/{count}) 正在核对完整性"),
            Language::English => format!("({index}/{count}) checking package integrity"),
        }
    }

    /// 「(i/N) 正在安装 <编号>」。
    pub(crate) fn installing_one(&self, index: u64, count: u64, id: &str) -> String {
        match self {
            Language::Chinese => format!("({index}/{count}) 正在安装 {id}"),
            Language::English => format!("({index}/{count}) installing {id}"),
        }
    }
}
