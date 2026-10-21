//! `memory.*` 的参数（施工 R-3 补，`docs/blueprint/memory.md`「协议」）：参数里不认识的格不理（`protocol.md`「请求」），
//! 写了 `as` 的不收（代表外部的人碰记忆随 O 线）。

use serde::Deserialize;
use serde_json::Value;

/// 指哪一间：`persona` 和 `session` 最多一个，都不写照默认人格。
#[derive(Debug, Deserialize)]
pub(super) struct Where {
    #[serde(default)]
    pub(super) persona: Option<String>,
    #[serde(default)]
    pub(super) session: Option<String>,
    /// 代表外部的人（`session.send` 的写法）：写了就不收。
    #[serde(default, rename = "as")]
    pub(super) as_external: Option<Value>,
}

/// `memory.list`。
#[derive(Debug, Deserialize)]
pub(super) struct List {
    #[serde(flatten)]
    pub(super) at: Where,
    #[serde(default)]
    pub(super) class: Option<String>,
    /// 出处在这个会话里的。
    #[serde(default)]
    pub(super) from: Option<String>,
    #[serde(default)]
    pub(super) forgotten: bool,
    #[serde(default)]
    pub(super) limit: Option<usize>,
}

/// `memory.search`。
#[derive(Debug, Deserialize)]
pub(super) struct Search {
    #[serde(flatten)]
    pub(super) at: Where,
    pub(super) query: String,
    #[serde(default)]
    pub(super) forgotten: bool,
    #[serde(default)]
    pub(super) limit: Option<usize>,
}

/// `memory.turns`（施工 R-12 下）。
#[derive(Debug, Deserialize)]
pub(super) struct Turns {
    #[serde(flatten)]
    pub(super) at: Where,
    pub(super) query: String,
    #[serde(default)]
    pub(super) limit: Option<usize>,
}

/// `memory.remember`。
#[derive(Debug, Deserialize)]
pub(super) struct Remember {
    #[serde(flatten)]
    pub(super) at: Where,
    pub(super) class: String,
    pub(super) text: String,
    /// 只用来检索的关键词（施工 R-15 上），可以不写。
    #[serde(default)]
    pub(super) keywords: Option<String>,
    /// 说的是谁（施工 E-1 上）：类是 `person` 的要写，别的类不看。
    #[serde(default)]
    pub(super) who: Option<String>,
}

/// `memory.update`：`text`、`keywords` 至少写一个（施工 R-15 上）。
#[derive(Debug, Deserialize)]
pub(super) struct Update {
    #[serde(flatten)]
    pub(super) at: Where,
    pub(super) id: String,
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) keywords: Option<String>,
    /// 换是谁（施工 E-1 上）：认识的人那一类才看。
    #[serde(default)]
    pub(super) who: Option<String>,
}

/// `memory.forget`：`id`（可以带 `why`）和 `clear` 两样只能写一样。
#[derive(Debug, Deserialize)]
pub(super) struct Forget {
    #[serde(flatten)]
    pub(super) at: Where,
    #[serde(default)]
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) why: Option<String>,
    #[serde(default)]
    pub(super) clear: Option<String>,
}

/// `memory.restore`（施工 R-12 下）。
#[derive(Debug, Deserialize)]
pub(super) struct Restore {
    #[serde(flatten)]
    pub(super) at: Where,
    pub(super) id: String,
}
