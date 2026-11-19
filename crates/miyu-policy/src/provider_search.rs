//! 供应商自己的联网工具换掉 Miyu 的搜索工具（设计 `72-DeepSeek内置联网工具.md` 第二节，施工 DSR-3）：判断结果钉在策略
//! 快照里，一个会话的工具面不因为中途改了「联网服务」变。
//!
//! 这里只有判断和筛工具面，纯逻辑：模型这边满不满足条件由会话的模型端口查（供应商、官方地址、驱动、思考没关），交进来
//! 的是 [`SearchOffer`]；「联网服务」选了什么由会话读配置交进来。

use serde::{Deserialize, Serialize};

use crate::ToolEntry;

/// 模型这边给得了的（供应商档案里写的，`resources/models/profiles.toml` 的 `search`）：「联网服务」选了 `setting` 才用，
/// `replaces` 是「Miyu 的工具、换成的供应商侧工具」，照档案的先后。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOffer {
    /// `net.search.source` 的哪个值算选了它。
    pub setting: String,
    /// Miyu 的工具名、换成的供应商侧工具名，照先后。
    pub replaces: Vec<(String, String)>,
}

/// 快照里钉的判断结果。只有模型这边满足条件时才有：不满足的会话没有这一格，快照的字节和以前一样。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchPin {
    /// 判断时会话的模型引用（模型或 `@池`）。回合开始时引用和它不一样就重新判断。
    pub model: String,
    /// 判断时「联网服务」选没选这一家。会话中途只改设置的不重新判断，所以要记下来。
    pub active: bool,
    /// 换掉的几件，照档案的先后：工具面里原本有的才换。没激活的、一件都没换的是空的，不写。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared: Vec<Replaced>,
}

/// 换掉的一件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replaced {
    /// Miyu 的工具名：不上工具面了。
    pub tool: String,
    /// 换成的供应商侧工具名：发请求时声明在 `tools` 的末尾。
    pub with: String,
}

impl SearchPin {
    /// 回合开始时照这一刻判断，交回要钉的。
    ///
    /// - `old`：快照里钉着的。
    /// - `model`：会话这时的模型引用。
    /// - `offer`：模型这边这一刻给不给得了，给不了的没有。
    /// - `picked`：这一轮的配置里「联网服务」选的值。
    /// - `face`：照这一刻的目录、预设筛出来、还没换掉任何一件的工具面。
    ///
    /// 模型这边给不了的不钉。给得了的：引用没变、原来钉着的，照原来钉的「选没选」（中途只改设置不算数）；引用变了、原来
    /// 没钉的，照 `picked` 重新判断。换哪几件每次照 `face` 重算：预设关掉了其中一件的那一件不换。
    pub fn judged(
        old: Option<&SearchPin>,
        model: &str,
        offer: Option<&SearchOffer>,
        picked: &str,
        face: &[ToolEntry],
    ) -> Option<SearchPin> {
        let offer = offer?;
        let active = match old {
            Some(pin) if pin.model == model => pin.active,
            _ => picked == offer.setting,
        };
        let declared = match active {
            true => offer
                .replaces
                .iter()
                .filter(|(tool, _)| face.iter().any(|entry| entry.name == *tool))
                .map(|(tool, with)| Replaced {
                    tool: tool.clone(),
                    with: with.clone(),
                })
                .collect(),
            false => Vec::new(),
        };
        Some(SearchPin {
            model: model.to_string(),
            active,
            declared,
        })
    }

    /// 发请求时要声明的供应商侧工具名，照先后。
    pub fn names(&self) -> Vec<String> {
        self.declared
            .iter()
            .map(|replaced| replaced.with.clone())
            .collect()
    }
}

/// 工具面里去掉 `pin` 换掉的那几件；没钉的、一件都没换的照原样。
pub fn strip(face: Vec<ToolEntry>, pin: Option<&SearchPin>) -> Vec<ToolEntry> {
    let Some(pin) = pin.filter(|pin| !pin.declared.is_empty()) else {
        return face;
    };
    face.into_iter()
        .filter(|entry| {
            !pin.declared
                .iter()
                .any(|replaced| replaced.tool == entry.name)
        })
        .collect()
}

#[cfg(test)]
mod tests;
