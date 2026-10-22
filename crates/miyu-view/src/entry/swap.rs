//! 换了模型的那一条（施工 9-8 十三补从 `entry.rs` 挪出来：那一份快到行数上限了）；换了人格、预设的那一条（施工 9-8 十四补）。

use serde::Serialize;

/// 换了模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelSwap {
    /// 换成的端点。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// 换成的模型。
    pub model: String,
    /// 换之前的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Was>,
    /// 为什么：`failover`、`replaced`。
    pub why: &'static str,
    /// 出错换的：换之前最近一次出错的分类。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
}

/// 换之前的模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Was {
    /// 端点。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// 模型。
    pub model: String,
}

/// 中途换了人格、预设（施工 9-8 十四补）：换了哪样写哪样。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recast {
    /// 人格。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<Swapped>,
    /// 预设。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<Swapped>,
}

/// 从哪个换成哪个：编号，没有的（无人格）是 `null`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Swapped {
    /// 换之前的。
    pub from: Option<String>,
    /// 换成的。
    pub to: Option<String>,
}
