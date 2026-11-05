//! 装、升级的确认屏说清来源和能力的字（施工 PK-3 下，设计 `61-装包说清来源和能力.md` 第三节，`docs/blueprint/cli/pkg.md`「给人看
//! 的字」）：来源的几种说法、能力一项一行、批过的标记、「注意：」、带着批准问的那几句、批了哪些。能力的名字、说明和「注意」后面
//! 的句子是核心的字（`resources/core/human`），照原样接在这些字里。

use super::super::{Doing, Language};

impl Language {
    /// 从软件源装的「来源」（施工 PK-3 下）：官方的「官方，签名已验证」；官方源里社区收录的「社区，签名已验证」，别人加的源写
    /// 源的名字 `feed`；不是官方的另接来源仓库 `origin`（核心给了的）。
    pub(crate) fn verified_from(&self, source: &str, feed: &str, origin: Option<&str>) -> String {
        let official = source == "official";
        let who = match (self, official, feed) {
            (Language::Chinese, true, _) => "官方",
            (Language::English, true, _) => "official",
            (Language::Chinese, false, "official") => "社区",
            (Language::English, false, "official") => "community",
            (_, false, feed) => feed,
        };
        let said = match self {
            Language::Chinese => format!("{who}，签名已验证"),
            Language::English => format!("{who}, signature verified"),
        };
        match (self, origin.filter(|_| !official)) {
            (_, None) => said,
            (Language::Chinese, Some(origin)) => format!("{said}（{origin}）"),
            (Language::English, Some(origin)) => format!("{said} ({origin})"),
        }
    }

    /// 不经软件源装的「来源」（施工 PK-3 下）：本机文件夹、包文件（`file`），装回出厂的（`shipped`），系统加装的（`distro`）。
    /// 别的（旧的核心不写、认不出的）没有。
    pub(crate) fn local_source(&self, source: &str) -> Option<&'static str> {
        Some(match (self, source) {
            (Language::Chinese, "file") => "本机文件",
            (Language::Chinese, "shipped") => "出厂",
            (Language::Chinese, "distro") => "系统的包管理器",
            (Language::English, "file") => "local file",
            (Language::English, "shipped") => "shipped with Miyu",
            (Language::English, "distro") => "system package manager",
            _ => return None,
        })
    }

    /// 能力一项（施工 PK-3 下）：核心给的名字接一句说明（没给说明的只有名字），批过、装完照旧留着的接「（已批准）」。
    pub(crate) fn capability(&self, name: &str, summary: Option<&str>, held: bool) -> String {
        let mut said = match (self, summary) {
            (_, None) => name.to_string(),
            (Language::Chinese, Some(summary)) => format!("{name}：{summary}"),
            (Language::English, Some(summary)) => format!("{name}: {summary}"),
        };
        if held {
            said.push_str(match self {
                Language::Chinese => "（已批准）",
                Language::English => " (approved)",
            });
        }
        said
    }

    /// 「注意：」接核心给的几句（施工 PK-3 下）：中文的句子自带句号、直接接，英文的空一格。
    pub(crate) fn notice(&self, sentences: &[&str]) -> String {
        match self {
            Language::Chinese => format!("注意：{}", sentences.concat()),
            Language::English => format!("Note: {}", sentences.join(" ")),
        }
    }

    /// 升级确认屏里每个包下面那一行的开头（施工 PK-3 下）：新版多要的能力、一起装的新依赖要的能力。缩进两格。
    pub(crate) fn upgrade_needs(&self, new: bool) -> &'static str {
        match (self, new) {
            (Language::Chinese, true) => "  新要：",
            (Language::Chinese, false) => "  需要：",
            (Language::English, true) => "  New: ",
            (Language::English, false) => "  Needs: ",
        }
    }

    /// 有要批的能力时问的那一句，后面不换行（施工 PK-3 下）：装、升级一个包的。
    pub(crate) fn proceed_approving(&self, doing: Doing, id: &str) -> String {
        match (self, doing) {
            (Language::Chinese, Doing::Upgrade) => format!("升级 '{id}' 并批准新要的能力？[Y/n] "),
            (Language::Chinese, _) => format!("安装 '{id}' 并批准它要的能力？[Y/n] "),
            (Language::English, Doing::Upgrade) => {
                format!("Upgrade '{id}' and approve the new capabilities? [Y/n] ")
            }
            (Language::English, _) => {
                format!("Install '{id}' and approve the capabilities it needs? [Y/n] ")
            }
        }
    }

    /// 同 [`Language::proceed_approving`]，几个包的：装 `count` 个、升级 `count` 个。
    pub(crate) fn proceed_many_approving(&self, doing: Doing, count: usize) -> String {
        match (self, doing) {
            (Language::Chinese, Doing::Upgrade) => {
                format!("升级 {count} 个软件包并批准新要的能力？[Y/n] ")
            }
            (Language::Chinese, _) => format!("安装 {count} 个软件包并批准它们要的能力？[Y/n] "),
            (Language::English, Doing::Upgrade) => {
                format!("Upgrade {count} packages and approve the new capabilities? [Y/n] ")
            }
            (Language::English, _) => {
                format!("Install {count} packages and approve the capabilities they need? [Y/n] ")
            }
        }
    }

    /// 推荐的里有要批的能力时接着问的那一句，后面不换行（施工 PK-3 下）。
    pub(crate) fn proceed_recommended_approving(&self, count: usize) -> String {
        match (self, count) {
            (Language::Chinese, 1) => {
                "同时安装推荐的 1 个软件包并批准它要的能力？[Y/n] ".to_string()
            }
            (Language::Chinese, _) => {
                format!("同时安装推荐的 {count} 个软件包并批准它们要的能力？[Y/n] ")
            }
            (Language::English, 1) => {
                "Also install 1 recommended package and approve the capabilities it needs? [Y/n] "
                    .to_string()
            }
            (Language::English, _) => format!(
                "Also install {count} recommended packages and approve the capabilities they need? [Y/n] "
            ),
        }
    }

    /// 装好、升好以后一个包一行（施工 PK-3 下，设计 61 定了的 6：`--yes` 也算批，照样列出批了哪些）：`names` 是这一次新批的
    /// 能力的名字，已经连好。
    pub(crate) fn approved(&self, id: &str, names: &str) -> String {
        match self {
            Language::Chinese => format!("已批准 {id} 的能力：{names}"),
            Language::English => format!("approved capabilities for {id}: {names}"),
        }
    }
}
