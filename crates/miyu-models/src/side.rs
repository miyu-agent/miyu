//! 旁路请求登记（施工 Q-2，设计 `43-旁路请求和档位池.md` 第三节第 2、3 条）：不在对话里的那些模型请求各自声明要哪一档，
//! 照档位从三个固定池（[`crate::pools::FIXED`]）里取；池是空的，用会话的模型；池的成员全在冷却、都出错了，也退到会话的模型
//! （项目主人 2026-10-21 定）。人管的只是三个池里放哪些模型，不另设每条请求的配置键（10-22 定）。
//!
//! 核心自己的几条写在这里（[`SHIPPED`]）；扩展的写在清单里（Q-3）。回顾、压缩摘要不登记：它们复用会话前缀的缓存，换个模型
//! 要整段原价重算。识图、向量是「用途」，照旧单配。

use miyu_config::Values;

use crate::pools;

/// 档位：照池的名字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// 便宜、快：起标题这类短的。
    Lite,
    /// 一般：要读懂一整段对话的（整理记忆）。
    Normal,
    /// 最好的。
    Flagship,
}

impl Tier {
    /// 这一档的池名：三个固定池之一。
    pub fn pool(self) -> &'static str {
        match self {
            Tier::Lite => pools::FIXED[0],
            Tier::Normal => pools::FIXED[1],
            Tier::Flagship => pools::FIXED[2],
        }
    }
}

/// 一条登记：编号（`<包>/<请求>`）、默认要哪一档。给人看的名字、说明在资源的 `model/side/<编号>` 下。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Side {
    /// 编号：`core/title` 这样。
    pub id: &'static str,
    /// 默认要哪一档。
    pub tier: Tier,
}

/// 起标题：只看第一问一答，短；起错了人能改。
pub const TITLE: Side = Side {
    id: "core/title",
    tier: Tier::Lite,
};

/// 整理记忆：抽取、合并、标签、整理会话都是它；要读懂一整段对话，太弱的会记错。
pub const ORGANIZE: Side = Side {
    id: "memory/organize",
    tier: Tier::Normal,
};

/// 核心出厂的登记，照编号排。
pub const SHIPPED: [Side; 2] = [TITLE, ORGANIZE];

/// 这一刻会落到哪（`model.list` 的 `resolved`）：登记的档位池解析得出成员的是池名；池是空的（一个都认不出）用会话的模型，
/// 交回空的。成员都在冷却这一层不看（路由那一层的事）。
pub fn resolved(values: &Values, side: &Side) -> Option<&'static str> {
    let pool = side.tier.pool();
    pools::pool(values, pool).ok().map(|_| pool)
}

/// 照档位用池的那一个引用：`@<池>`。
pub fn reference(side: &Side) -> String {
    format!("@{}", side.tier.pool())
}

#[cfg(test)]
mod tests;
