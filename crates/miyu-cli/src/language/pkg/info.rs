//! `miyu pkg info` 的字（施工 IN-2，设计 `49-装包照pacman.md` 第一节，`docs/blueprint/cli/pkg.md`「给人看的字」）：每一行前面
//! 的键（英文照 pacman）、没有依赖时写的「无」、来源的几种说法、哪儿都没有的报错；`-Qi`、`-Si` 接了路径的报错（施工 IN-4）。

use super::super::Language;
use crate::diagnostic::Problem;

/// `info` 的一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InfoKey {
    Repository,
    Name,
    Title,
    Version,
    Description,
    DependsOn,
    Recommends,
    Provides,
    /// 包目录、包文件里读不成的（施工 IN-4）。
    Skipped,
    Capabilities,
    /// 从软件源装时推荐的包要的能力（施工 PK-3 下）。
    RecommendedCapabilities,
    Origin,
    Author,
    Upstream,
    DownloadSize,
    InstalledSize,
    RequiredBy,
    InstallDate,
}

/// 包是从哪来的（「来源」那一行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CameFrom {
    Shipped,
    Official,
    Community,
    /// 装着的、从软件源装的：本地库只记了来源仓库，分不出是不是官方的源，不说官方。
    Source,
    Local,
}

impl Language {
    /// `info` 一行前面的键。
    pub(crate) fn info_key(&self, key: InfoKey) -> &'static str {
        let (zh, en) = match key {
            InfoKey::Repository => ("软件源", "Repository"),
            InfoKey::Name => ("编号", "Name"),
            InfoKey::Title => ("名称", "Title"),
            InfoKey::Version => ("版本", "Version"),
            InfoKey::Description => ("描述", "Description"),
            InfoKey::DependsOn => ("依赖", "Depends On"),
            InfoKey::Recommends => ("推荐", "Recommends"),
            InfoKey::Provides => ("提供", "Provides"),
            InfoKey::Skipped => ("无法读取", "Unreadable"),
            InfoKey::Capabilities => ("需要的能力", "Capabilities"),
            InfoKey::RecommendedCapabilities => ("推荐的包要的能力", "Recommended Capabilities"),
            InfoKey::Origin => ("来源", "Origin"),
            InfoKey::Author => ("作者", "Author"),
            InfoKey::Upstream => ("上游", "Upstream"),
            InfoKey::DownloadSize => ("下载大小", "Download Size"),
            InfoKey::InstalledSize => ("安装后大小", "Installed Size"),
            InfoKey::RequiredBy => ("被依赖", "Required By"),
            InfoKey::InstallDate => ("安装时间", "Install Date"),
        };
        match self {
            Language::Chinese => zh,
            Language::English => en,
        }
    }

    /// 「依赖」「推荐」没有的写的。
    pub(crate) fn info_none(&self) -> &'static str {
        match self {
            Language::Chinese => "无",
            Language::English => "None",
        }
    }

    /// 「来源」：出厂、官方、社区、本地；有来源仓库的写在仓库后面的括号里。
    pub(crate) fn came_from(&self, from: CameFrom, origin: Option<&str>) -> String {
        let word = match (self, from) {
            (Language::Chinese, CameFrom::Shipped) => "出厂",
            (Language::Chinese, CameFrom::Official) => "官方",
            (Language::Chinese, CameFrom::Community) => "社区",
            (Language::Chinese, CameFrom::Source) => "软件源",
            (Language::Chinese, CameFrom::Local) => "本地",
            (Language::English, CameFrom::Shipped) => "shipped",
            (Language::English, CameFrom::Official) => "official",
            (Language::English, CameFrom::Community) => "community",
            (Language::English, CameFrom::Source) => "repository",
            (Language::English, CameFrom::Local) => "local",
        };
        match (self, origin) {
            (_, None) => word.to_string(),
            (Language::Chinese, Some(origin)) => format!("{origin}（{word}）"),
            (Language::English, Some(origin)) => format!("{origin} ({word})"),
        }
    }

    /// `-Qi`（`source` 是 `false`）、`-Si`（`true`）接的是路径（报错，施工 IN-4）：看包目录、包文件用 `info`。
    pub(crate) fn info_takes_an_id(&self, source: bool) -> Problem {
        let flag = match source {
            true => "-Si",
            false => "-Qi",
        };
        match self {
            Language::Chinese => Problem::new(format!("{flag} 只接软件包编号"))
                .hint("运行 'miyu pkg info <路径>' 查看软件包目录或文件"),
            Language::English => Problem::new(format!("{flag} takes a package id"))
                .hint("run 'miyu pkg info <path>' to look at a package folder or file"),
        }
    }

    /// 装着的、软件源里都没有 `id`（报错，设计 47）。
    pub(crate) fn package_not_found(&self, id: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("未找到软件包 '{id}'")),
            Language::English => Problem::new(format!("package '{id}' not found")),
        }
    }
}
