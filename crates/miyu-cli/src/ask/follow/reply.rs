//! 一轮里每次回复、每次请求记下的（施工 AP-3 从 `follow.rs` 挪出来）：回复的正文给 `--format json`、事件流的 `turn.ended`；
//! 请求的用量加起来，出错的记下。

use serde_json::{Value, json};

use super::{Failure, Follow, Screen};

impl Follow<'_> {
    /// 最后写成的回复：正文块给 `--format json`。和前一段回复之间隔着步骤、前一段又没换行的，补一个换行，
    /// 和标准输出上的一样。
    pub(super) fn reply(&mut self, body: &Value) {
        let mut text = String::new();
        for block in body["blocks"].as_array().into_iter().flatten() {
            if block["type"] == json!("text")
                && let Some(piece) = block["text"].as_str()
            {
                text.push_str(piece);
            }
        }
        if text.is_empty() {
            return;
        }
        if self.stepped && !self.answer.is_empty() && !self.answer.ends_with('\n') {
            self.answer.push('\n');
        }
        self.stepped = false;
        self.answer.push_str(&text);
    }

    /// 一次请求的记录：用量加起来；出错的记下，后来又成了的（重试）当没出过。压缩的摘要请求出错的，说压缩失败
    /// （施工 6-3 下）。
    pub(super) fn called(&mut self, body: &Value, screen: &mut Screen<'_>) {
        self.compaction_called(body, screen);
        self.usage.add(&body["usage"]);
        self.total.add(&body["usage"]);
        self.failure = match body["result"].as_str() {
            Some("error") => Some(Failure {
                class: body["error"]["class"]
                    .as_str()
                    .unwrap_or("other")
                    .to_string(),
                message: body["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                sent: !body["endpoint"].is_null(),
            }),
            _ => None,
        };
    }
}
