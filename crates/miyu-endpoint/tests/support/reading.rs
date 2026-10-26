//! 读核心写来的（施工 V-19 从 `mod.rs` 挪出来，那一份到了行数上限）：读下一行；发一条请求、读它的回应。回应要等核心同步完才交，
//! 可能排在后面的推送后头（`protocol.md`「对外的样子」）：等回应时读到的别的先存着，下一次读照先后先交它们，一条都不丢。

use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::AsyncBufReadExt;

use super::Client;

impl Client {
    /// 读下一条：先交等回应时存着的，再读下一行，认成 JSON；对方关了的是 `None`。最多等六十秒（原来十秒，机器忙时会超过，
    /// 2026-10-07）。
    pub async fn next(&mut self) -> Option<Value> {
        if let Some(held) = self.held.pop_front() {
            return Some(held);
        }
        self.read().await
    }

    /// 读下一条，最多等 `wait`：先交存着的；等不到的是 `None`（对方关了的也是）。
    pub async fn next_within(&mut self, wait: Duration) -> Option<Value> {
        if let Some(held) = self.held.pop_front() {
            return Some(held);
        }
        let mut line = String::new();
        match tokio::time::timeout(wait, self.reader.read_line(&mut line)).await {
            Ok(Ok(read)) if read > 0 => Some(serde_json::from_str(&line).expect("是 JSON")),
            _ => None,
        }
    }

    /// 发一条请求，读它的回应：读到编号是 `id` 的那一条为止，路上读到的别的存着，留给之后的 [`Client::next`]。
    pub async fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.line(&request.to_string()).await;
        loop {
            let next = self.read().await.expect("有回应");
            if next["id"] == json!(id) {
                return next;
            }
            self.held.push_back(next);
        }
    }

    /// 从连接上读下一行，认成 JSON；对方关了的是 `None`。最多等六十秒。
    async fn read(&mut self) -> Option<Value> {
        let mut line = String::new();
        let read = tokio::time::timeout(Duration::from_secs(60), self.reader.read_line(&mut line))
            .await
            .expect("六十秒内有回应")
            .expect("读得了");
        (read > 0).then(|| serde_json::from_str(&line).expect("回应是 JSON"))
    }
}
