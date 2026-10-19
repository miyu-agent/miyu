//! 换了模型的那一条（施工 9-8 十三补从 `entry.rs` 挪出来：那一份快到行数上限了）。

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
