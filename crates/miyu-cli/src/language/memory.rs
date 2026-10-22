//! `miyu memory` 给人看的字（施工 R-3 再补，`docs/blueprint/cli/memory.md`「给人看的字」）。

use super::Language;

impl Language {
    /// `list` 一条都没有。
    pub fn no_memories(&self) -> &'static str {
        match self {
            Language::Chinese => "还没有记忆。",
            Language::English => "No memories yet.",
        }
    }

    /// `cards` 一张都没有（施工 E-1 下）。
    pub fn no_cards(&self) -> &'static str {
        match self {
            Language::Chinese => "还没有认识的人的卡。",
            Language::English => "No person cards yet.",
        }
    }

    /// `search` 一条都没找到。
    pub fn nothing_found(&self) -> &'static str {
        match self {
            Language::Chinese => "没找到。",
            Language::English => "Nothing found.",
        }
    }

    /// `add` 记成了 `id`。
    pub fn remembered(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("记下了：{id}"),
            Language::English => format!("Remembered as {id}"),
        }
    }

    /// `edit` 改好了，新的一条是 `id`。
    pub fn changed(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("改好了：{id}"),
            Language::English => format!("Changed; now {id}"),
        }
    }

    /// 清空清掉了 `count` 条。
    pub fn cleared(&self, count: u64) -> String {
        match self {
            Language::Chinese => format!("清掉了 {count} 条。"),
            Language::English => format!("Cleared {count}."),
        }
    }

    /// `dream` 整理完了（施工 R-7 补，和 `/dream` 的回执同一句；R-22 三补照它改成是 0 的不写、末尾写用时）：一条都没看的说
    /// 没有要整理的。
    pub fn dreamed(&self, organized: &Organized) -> String {
        let took = miyu_view::clock(organized.took_ms);
        match (self, organized.given) {
            (Language::Chinese, 0) => format!("无需整理，用时 {took}。"),
            (Language::English, 0) => format!("Nothing to organize; took {took}."),
            (Language::Chinese, _) => {
                format!(
                    "整理完成：{}，用时 {took}。",
                    self.organized(organized, true)
                )
            }
            (Language::English, _) => format!(
                "Memory organized: {}; took {took}.",
                self.organized(organized, true)
            ),
        }
    }

    /// 几样数拼成几段（同 `/dream` 的回执，施工 R-22 再补、三补）：检查几条照写；改了的、作废的是 0 的不写，都是 0 的说没有
    /// 要改的；`summary` 是摘要换了的多一段（叫停的不说）。
    fn organized(&self, organized: &Organized, summary: bool) -> String {
        let zh = matches!(self, Language::Chinese);
        let mut parts = vec![if zh {
            format!("检查 {} 条", organized.given)
        } else {
            format!("{} checked", organized.given)
        }];
        if organized.revised > 0 {
            parts.push(if zh {
                format!("修改 {} 条", organized.revised)
            } else {
                format!("{} revised", organized.revised)
            });
        }
        if organized.retired > 0 {
            parts.push(if zh {
                format!("作废 {} 条", organized.retired)
            } else {
                format!("{} retired", organized.retired)
            });
        }
        if organized.revised == 0 && organized.retired == 0 {
            parts.push(
                if zh {
                    "没有要改的"
                } else {
                    "nothing to change"
                }
                .to_string(),
            );
        }
        if summary && organized.summary {
            parts.push(
                if zh {
                    "摘要已更新"
                } else {
                    "summary updated"
                }
                .to_string(),
            );
        }
        parts.join(if zh { "，" } else { ", " })
    }

    /// `add --class person` 没写 `--who`（施工 E-1 上）：认识的人要写是谁。
    pub(crate) fn memory_needs_who(&self) -> &'static str {
        match self {
            Language::Chinese => "认识的人要写 --who（平台编号或名字）。",
            Language::English => "A person memory needs --who (a platform id or a name).",
        }
    }

    /// `dream` 第一次按 Ctrl+C（施工 R-23）：在叫停，再按一次直接退。
    pub(crate) fn dream_stopping(&self) -> &'static str {
        match self {
            Language::Chinese => "正在停止整理……再按一次 Ctrl+C 直接退出。",
            Language::English => "Stopping… press Ctrl+C again to quit now.",
        }
    }

    /// `dream` 叫停了（施工 R-23，和 `/dream` 叫停了的回执同一句）：叫停以前合完了的几样数，末尾写用时；一批都没合完的只说
    /// 中断了。
    pub(crate) fn dream_stopped(&self, organized: &Organized) -> String {
        let took = miyu_view::clock(organized.took_ms);
        match (self, organized.given) {
            (Language::Chinese, 0) => format!("整理已中断，用时 {took}，下次再整理。"),
            (Language::English, 0) => {
                format!("Memory organizing stopped after {took}; it will continue next time.")
            }
            (Language::Chinese, _) => format!(
                "整理已中断：{}，用时 {took}，其余下次再整理。",
                self.organized(organized, false)
            ),
            (Language::English, _) => format!(
                "Memory organizing stopped: {}; took {took}; the rest next time.",
                self.organized(organized, false)
            ),
        }
    }

    /// 记的、改的太长：这一条 `chars` 个字，一条最多 `limit` 个。
    pub(crate) fn too_long(&self, chars: u64, limit: u64) -> String {
        match self {
            Language::Chinese => format!("太长了：这一条 {chars} 个字，一条最多 {limit} 个字。"),
            Language::English => {
                format!(
                    "Too long: this one has {chars} characters, a memory takes at most {limit}."
                )
            }
        }
    }

    /// 作废了的一条接在正文后面的那一截：`why` 是空的只说作废了。
    pub(crate) fn forgotten_mark(&self, why: &str) -> String {
        match (self, why.is_empty()) {
            (Language::Chinese, true) => "（已作废）".to_string(),
            (Language::Chinese, false) => format!("（已作废：{why}）"),
            (Language::English, true) => " (forgotten)".to_string(),
            (Language::English, false) => format!(" (forgotten: {why})"),
        }
    }
}

/// 一趟整理的几样数（施工 R-22 三补）：`memory.dream` 回应里的，加上命令行自己量的用时。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Organized {
    /// 检查了几条。
    pub given: u64,
    /// 改了几条。
    pub revised: u64,
    /// 作废几条。
    pub retired: u64,
    /// 摘要换了。
    pub summary: bool,
    /// 用了多久，毫秒。
    pub took_ms: u64,
}
