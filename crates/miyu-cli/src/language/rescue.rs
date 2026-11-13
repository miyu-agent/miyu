//! `miyu rescue` 给人看的字（施工 RC-1，`docs/blueprint/cli/rescue.md`「给人看的字」）：照设计 47 第三节，报错、警告由
//! [`crate::diagnostic`] 加程序名、去句号。别处交来的原话（读配置的原因）里的控制字符换成 `�`：可能混着终端的控制序列。

use miyu_store::human::clean;

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// 找不到 `litemiyu`：`linux` 为真的说怎么装，别的系统说暂只支持 Linux（LiteMiyu 现在只发 Linux 的包）。
    pub(crate) fn rescue_missing(&self, linux: bool) -> Problem {
        let (what, hint) = match (self, linux) {
            (Language::Chinese, true) => (
                "未找到 litemiyu",
                "安装：Arch Linux 用 AUR（paru -S litemiyu 或 yay -S litemiyu）；其他发行版从 https://github.com/miyu-agent/litemiyu/releases 下载",
            ),
            (Language::Chinese, false) => ("未找到 litemiyu", "暂只支持 Linux"),
            (Language::English, true) => (
                "litemiyu not found",
                "to install it, use the AUR on Arch Linux (paru -S litemiyu or yay -S litemiyu); on other distributions, download it from https://github.com/miyu-agent/litemiyu/releases",
            ),
            (Language::English, false) => ("litemiyu not found", "only Linux is supported for now"),
        };
        Problem::new(what).hint(hint)
    }

    /// 算不出 Miyu 的对话模型（`why` 是原话）：一个变量都不填，照常拉起，由 LiteMiyu 照它自己的配置找模型。
    pub(crate) fn rescue_unconfigured(&self, why: &str) -> Problem {
        let why = clean(why);
        Problem::new(match self {
            Language::Chinese => format!("读不了 Miyu 的配置：{why}，交给 litemiyu 自己找模型"),
            Language::English => {
                format!(
                    "cannot read Miyu's configuration: {why}; litemiyu will find a model itself"
                )
            }
        })
    }

    /// 对话模型走扩展驱动（`driver` 是 `<包>/<驱动名>`）：LiteMiyu 认不了，一个变量都不填，照常拉起（core1 2026-11-13 定）。
    pub(crate) fn rescue_extension(&self, driver: &str) -> Problem {
        let driver = clean(driver);
        Problem::new(match self {
            Language::Chinese => {
                format!("对话模型用的是扩展驱动 {driver}，litemiyu 认不了，交给它自己找模型")
            }
            Language::English => format!(
                "the chat model uses the extension driver {driver}, which litemiyu cannot use; litemiyu will find a model itself"
            ),
        })
    }
}
