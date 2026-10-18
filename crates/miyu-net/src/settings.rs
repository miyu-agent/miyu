//! 联网这个包的配置项（施工 N-2 下，设计 36 第三节第 1 条，项目主人 2026-10-18 定）：设置页一个「搜索来源」下拉。
//!
//! `auto` 照出厂的顺序（`search.json` 的 `order`）；选了一家的一直先用它，它用不了（出错、限额冷却着）才照出厂的顺序兜底
//! （第二节第 3 条）。下一轮生效：会话 actor 回合开始冻结配置，`web_search` 照调用带来的那一份读（`miyu_tool::SettingsPort`）。
//! 核心替联网这个内置包声明，包没装的设置页不画（`miyu-core` 的 `settings::Packaged`）。

miyu_config::settings! {
    /// 搜索的配置。
    pub struct SearchSettings in "net.search" {
        /// 搜索来源：`auto` 照出厂的顺序，或者先用定的那一家。
        source: String = "auto" {
            kind: option ["auto", "exa", "parallel", "firecrawl", "anysearch", "you"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "net", control: select },
        },
    }
}

/// 不挑、照出厂顺序的那一个选项。
pub const AUTO: &str = "auto";

#[cfg(test)]
mod tests;
