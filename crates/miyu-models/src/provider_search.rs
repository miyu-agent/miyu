//! 供应商那边自带的联网搜索（设计 `72-DeepSeek内置联网工具.md`，施工 DSR-3）：档案里一家供应商的 `search` 一格写着它的样子，
//! [`offered`] 回答这一次请求能不能用它。
//!
//! 档案的写法（`resources/models/profiles.toml` 的 `[providers.deepseek]`）：
//!
//! ```toml
//! search = { setting = "deepseek-responses", driver = "openai-responses", effort = "high", replaces = [["web_search", "deepseek_web_search"]] }
//! ```
//!
//! 意思是：配置项 `net.search.source` 的值是 `setting`，并且这一家在官方地址上、这个模型真走的驱动是 `driver`、思考没有关闭时，
//! Miyu 自己的工具（每一对的前一个名字）换成供应商那边的工具声明（后一个名字），声明照 `replaces` 的先后接在工具面的末尾。
//! 配置项的值由调的一方比，这里只管供应商、驱动、思考这三样。

use serde::Deserialize;

use crate::provider::Provider;
use crate::provider_key::official_address;

/// 档案里一家供应商自带的联网搜索（设计 72 第二节）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSearch {
    /// 配置项 `net.search.source` 选中它时的值，例如 `deepseek-responses`。
    pub setting: String,
    /// 只有说这种驱动时才有，例如 `openai-responses`：DeepSeek 的联网工具只在 Responses 接口上有（设计 72 第二节第 3 条）。
    pub driver: String,
    /// 声明了供应商那边的工具、这一次又没有定思考强度时，请求里明着写的那一档，例如 `high`。DeepSeek 默认的强度就是它，
    /// 可是普通工具和联网工具一起声明时不明着写，服务端不会真去搜索（2026-11-19 实测，设计 72 第三节）。
    pub effort: String,
    /// 换哪几件：每一对是 Miyu 自己的工具名和供应商那边的工具声明的名字，供应商那边的照这个先后接在工具面的末尾。
    pub replaces: Vec<(String, String)>,
}

impl ProviderSearch {
    /// 查写法：每一格都不能是空的字，同一个名字在每一对的同一边只能出现一次。
    ///
    /// # Errors
    ///
    /// 有空的字，或者同一边有重复的名字：原因写明是哪一格。
    pub fn check(&self) -> Result<(), String> {
        if self.setting.is_empty() || self.driver.is_empty() || self.effort.is_empty() {
            return Err("search needs a non-empty setting, driver and effort".to_string());
        }
        let own: Vec<&str> = self.replaces.iter().map(|(own, _)| own.as_str()).collect();
        let theirs: Vec<&str> = self
            .replaces
            .iter()
            .map(|(_, theirs)| theirs.as_str())
            .collect();
        for (which, names) in [("own", own), ("provider", theirs)] {
            for (at, name) in names.iter().enumerate() {
                if name.is_empty() {
                    return Err(format!("search replaces with an empty {which} tool name"));
                }
                if names[..at].contains(name) {
                    return Err(format!("search names {which} tool {name:?} twice"));
                }
            }
        }
        Ok(())
    }
}

/// 这一次请求能用供应商那边的联网搜索时，交回档案里的那一格（设计 72 第二节的激活条件）。这几样都成立才交：
///
/// 1. 这一家的档案写了 `search`；
/// 2. 地址是这一家的官方地址（[`official_address`]，和交 key、查余额同一个判断）：经过中转站的、地址是环境变量引用的都不算，
///    官方地址表里没有的一家也不算；
/// 3. 这个模型真走的驱动是 `search.driver`；
/// 4. 思考没有关闭：强度 `effort` 不是 [`miyu_drivers::EFFORT_OFF`]。DeepSeek 同时收到普通工具和联网工具、思考又关着时回 400
///    （设计 72 第四节第 1 条）。没有强度的算没关。
///
/// `speaking` 是照 [`Provider::for_model`] 换过的这一家：驱动、地址都是真发的那一个模型的。`effort` 是这一次规整过的强度
/// （和 `Call.effort` 同一个）。
pub fn offered<'a>(speaking: &'a Provider, effort: Option<&str>) -> Option<&'a ProviderSearch> {
    let search = speaking.search.as_ref()?;
    let official = official_address(Some(&speaking.base_url), &speaking.catalog);
    let driver = speaking.driver.as_str() == search.driver;
    let thinking = effort != Some(miyu_drivers::EFFORT_OFF);
    (official && driver && thinking).then_some(search)
}

#[cfg(test)]
mod tests;
