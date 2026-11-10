//! 一行太长（施工 YC-5，压测 F5、O11）。

use serde_json::json;

use super::{Locale, REFUSED, Refusal};
use crate::wire::LINE_LIMIT;

impl Refusal {
    /// 收到的一行超过 [`LINE_LIMIT`]：没有解析，只拒这一条，连接照旧。`data.limit` 是上限（字节）。原来回 `parse_error`
    /// 「消息格式错误」，和日志里的原因对不上。
    pub(crate) fn line_too_long() -> Refusal {
        let mut refusal = Refusal {
            code: REFUSED,
            reason: "line_too_long",
            data: None,
        };
        refusal
            .data
            .get_or_insert_with(serde_json::Map::new)
            .insert("limit".to_string(), json!(LINE_LIMIT));
        refusal
    }

    /// 对方答核心的那一行太长时，交给等这条回应的一方的错误（施工 YC-5补）：写成对方自己回了一条 JSON-RPC 错误的样子，
    /// 各处照「对方回了错」的路走。字的码照 `error.data.code` 放（施工 BUS-2 的写法，`package.call` 照它转进
    /// `method_failed` 的 `data.code`），`data` 里别的照 `line_too_long` 自己的；那一句用英文的：这时不知道最后看的人用哪种语言。
    pub(crate) fn answered(&self) -> serde_json::Value {
        let mut data = self.data.clone().unwrap_or_default();
        data.insert("code".to_string(), json!(self.reason));
        json!({"code": self.code, "message": self.message(Locale::En), "data": data})
    }
}
