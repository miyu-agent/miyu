//! 一行太长（施工 YC-5，压测 F5、O11）。

use serde_json::json;

use super::{REFUSED, Refusal};
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
}
