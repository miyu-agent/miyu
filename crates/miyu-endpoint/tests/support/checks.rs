//! 几样共用的查法（施工 X-3 下从 `mod.rs` 拆出来：那一份到了行数上限）：等到某件事成立、去掉订阅回应里「当前的」几格、
//! 拒绝的原因码。

use std::time::Duration;

use serde_json::Value;

/// 等到 `done` 成立，最多六十秒。
pub async fn until(what: &str, done: impl Fn() -> bool) {
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "六十秒内没等到{what}");
}

/// 订阅的回应去掉「当前的」几格（施工 9-6 上：`usage`、`permission`、`jobs`；施工 9-7 上：`workspace`）和会话的默认工作区
/// （施工 5-14 中：`default_workspace`，照数据根的位置变）：只看限额、模型、补发这些的测试用，这几格另有
/// `tests/subscribe_now.rs`、`tests/limits.rs`、`tests/workspace_property.rs` 守着。
pub fn apart_from_now(result: &Value) -> Value {
    let mut result = result.clone();
    if let Some(fields) = result.as_object_mut() {
        for key in [
            "usage",
            "permission",
            "jobs",
            "workspace",
            "default_workspace",
        ] {
            fields.remove(key);
        }
    }
    result
}

/// 回应里的原因码；不是拒绝的是 `None`。
pub fn reason(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}
