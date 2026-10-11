//! 合并转发里的消息怎么读（施工 O-34，`onebot.md` 第一条「群消息」第 2 条、「施工时定的」第 206 条起）：取的动作
//! `get_forward_msg {id}`，回应的 `data.messages`；一条（节点）是谁发的、说了什么。NapCat 回的节点是一条完整的消息
//! （`sender`、`message` 段数组、`user_id`、`time`）；有的实现照发合并转发的写法回 `{type: "node", data: {user_id, nickname,
//! content}}`，两种都认。只认、不洗，洗在交给核心的一方（`core/route/forwards.rs`）。

use serde_json::{Value, json};

use super::{Segments, display_name, number, segments};

/// 取合并转发里的消息的动作和参数：`get_forward_msg {id}`，编号原样写成字（NapCat 认 `id`、`message_id`，数字的照消息编号找，
/// 别的照转发的资源编号找）。
pub fn get_forward_msg(id: &str) -> (&'static str, Value) {
    ("get_forward_msg", json!({"id": id}))
}

/// `get_forward_msg` 回应的 `data` 里的消息：`messages` 是数组的才有。
pub fn forward_messages(data: &Value) -> Option<Vec<Value>> {
    data["messages"].as_array().cloned()
}

/// 合并转发里的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// 发的人的号：`sender.user_id`，没有的照 `user_id`；都没有、读不出的是空的。
    pub user: Option<i64>,
    /// 发的人当时叫什么：`sender` 的群名片、昵称（同群消息的 `name`），没有的照节点自己的 `card`、`nickname`；照原样，没洗。
    pub name: Option<String>,
    /// 认出来的段，同一条消息的认法（`segments`）：`message`，没有的照 `content`。
    pub segments: Segments,
}

/// 读一条：`value` 是 NapCat 交的节点。
pub fn node(value: &Value) -> Node {
    let body = if value["type"] == "node" {
        &value["data"]
    } else {
        value
    };
    let message = if body["message"].is_null() {
        &body["content"]
    } else {
        &body["message"]
    };
    Node {
        user: number(&body["sender"]["user_id"]).or_else(|| number(&body["user_id"])),
        name: display_name(&body["sender"]).or_else(|| display_name(body)),
        segments: segments(message),
    }
}

#[cfg(test)]
mod tests;
