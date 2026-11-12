//! 读推送、读日志的几个小工具（施工 O-3 从 `mod.rs` 挪出来：那一份到了 500 行）。

use std::time::Duration;

use serde_json::{Value, json};

use super::{Client, Home};

/// 推送里的事件种类，照先后。
pub fn kinds(pushed: &[Value]) -> Vec<String> {
    pushed
        .iter()
        .filter(|push| push["method"] == json!("event"))
        .map(|push| {
            push["params"]["event"]["kind"]
                .as_str()
                .unwrap_or("?")
                .to_string()
        })
        .collect()
}

/// 推送里的事件，照先后（施工 3-8 六补）。
pub fn events(pushed: &[Value]) -> Vec<Value> {
    pushed
        .iter()
        .filter(|push| push["method"] == json!("event"))
        .map(|push| push["params"]["event"].clone())
        .collect()
}

/// 磁盘上会话 `session` 的日志，每条写成 JSON，照先后（施工 3-8 六补）：和推送里的比。
pub fn logged(home: &Home, session: &str) -> Vec<Value> {
    home.log(session)
        .iter()
        .map(|event| serde_json::from_str(&event.to_line()).expect("事件是 JSON"))
        .collect()
}

/// 读到一条 `package.changed`、编号是 `id` 为止，交回它的 `entry`：别的消息跳过，最多 60 秒（施工 PV-2 从 `packages_push.rs`
/// 挪过来，两个测试文件共用）。
pub async fn package_changed(client: &mut Client, id: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到 {id} 的推送"));
        if message["method"] == "package.changed" && message["params"]["package"] == id {
            return message["params"]["entry"].clone();
        }
    }
}
