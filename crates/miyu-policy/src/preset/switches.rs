//! 人在技能、脚本、MCP 三页上设的全局开关（施工 X-27，`docs/blueprint/presets.md`「全局开关」，2026-10-20 项目主人定）：记在
//! 个人设置的 `features.<功能编号>`，`unlisted` 开着的预设照它；预设里对这个功能明写了开或关的照预设，三页只是方便管理。
//!
//! - 「明写了」是叠好的文件里 `[features]` 写了这个功能，或者以前的 `[software]` 写了这个功能、它所在的包（[`By::Preset`]）；
//!   都没写的照全局开关（[`By::Global`]）；全局开关也没写的照 `unlisted`（[`By::Default`]）。
//! - 全局开关只管 `unlisted` 是开的预设（2026-10-20 项目主人定）：`unlisted = "off"` 的（出厂的 `dev`）没写的照旧关着，
//!   全局开关打开的也不开，记 [`By::Default`]。
//! - 只管技能、脚本、MCP 服务三类功能（[`switchable`]），包自己的功能不接。
//! - 开关不是预设文件里写的：核心照这一轮的配置放进叠好的文件（[`PresetFile::following`]），不进指纹、不叠层。改了以后没开的
//!   功能跟着变，快照里记的不一样了，下一个回合换上。

use std::collections::BTreeMap;

use super::{PresetFile, Unlisted};
use crate::features::Features;

/// 三类功能编号的开头：技能 `skill-<名字>`、脚本 `script-<工具名>`、MCP 服务 `mcp-<服务名>`。
const SWITCHABLE: [&str; 3] = ["skill-", "script-", "mcp-"];

/// 功能 `feature` 能不能用全局开关开关：是技能、脚本、MCP 服务的。
pub fn switchable(feature: &str) -> bool {
    SWITCHABLE.iter().any(|prefix| feature.starts_with(prefix))
}

/// 一个功能开不开是谁定的（`preset.get` 每一项的 `by`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// 预设里明写了。
    Preset,
    /// 预设没写，照人设的全局开关。
    Global,
    /// 都没写，照预设的 `unlisted`。
    Default,
}

impl By {
    /// 协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            By::Preset => "preset",
            By::Global => "global",
            By::Default => "default",
        }
    }
}

impl PresetFile {
    /// 放进这一轮这个人关了的包 `off`（施工 U23 末上，个人设置 `packages.<编号>.off`）：整份换掉原来的，`features` 记下的必需的
    /// 不算（[`PresetFile::switched_off`]）。改了从下一个回合开始生效。
    #[must_use]
    pub fn switching_off(
        mut self,
        off: impl IntoIterator<Item = String>,
        features: &Features,
    ) -> PresetFile {
        self.switched_off = off
            .into_iter()
            .filter(|id| !features.required().contains(id))
            .collect();
        self
    }

    /// 放进这一轮的全局开关 `global`（功能编号到开不开，照 `features.<功能编号>` 读的）：整份换掉原来的。
    #[must_use]
    pub fn following(mut self, global: BTreeMap<String, bool>) -> PresetFile {
        self.global = global;
        self
    }

    /// 包 `package` 里的功能 `feature` 开不开、是谁定的：预设明写了的照预设；没写的三类功能，`unlisted` 开着的照全局开关；
    /// 别的照 `unlisted`。不管扣下的包（[`PresetFile::opens_in`] 先挡）。
    pub fn decided(&self, feature: &str, package: &str) -> (bool, By) {
        let written = self
            .features
            .get(feature)
            .or_else(|| self.software.get(feature))
            .or_else(|| self.software.get(package));
        if let Some(on) = written {
            return (*on, By::Preset);
        }
        let unlisted = self.unlisted() == Unlisted::On;
        match self
            .global
            .get(feature)
            .filter(|_| unlisted && switchable(feature))
        {
            Some(on) => (*on, By::Global),
            None => (unlisted, By::Default),
        }
    }
}

#[cfg(test)]
mod tests;
