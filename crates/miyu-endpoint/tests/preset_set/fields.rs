//! 预设文件里的几格怎么改（施工 F-3 下，从 `preset_set.rs` 挪出来：那边放不下了）：照功能改；没人看着时的步数上限。

use serde_json::json;

use super::{connected, mine, on, set};
use crate::support::*;

/// 照功能改（施工 F-3 下）：`features.<编号>` 写进 `[features]`，回应里那个功能跟着变，别的照旧。
#[tokio::test]
async fn a_feature_is_switched_by_its_id() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let reply = set(
        &mut client,
        "f1",
        "full",
        json!([{"key": "features.shell", "value": false}]),
    )
    .await;
    assert_eq!(on(&reply, "shell"), Some(false), "{reply}");
    assert_eq!(on(&reply, "read"), Some(true));
    assert_eq!(
        mine(&home, "full").as_deref(),
        Some("[features]\nshell = false\n")
    );
}

/// 没人看着时的步数上限（施工 AE-1）：`limits.unattended_steps` 写进 `[limits]`，`preset.get` 回它；没写的是 `null`；0 整条不收；
/// `unset` 去掉回到 `null`。
#[tokio::test]
async fn the_unattended_step_limit_is_written_and_read_back() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let got = client
        .call("g0", "preset.get", json!({"preset": "dev"}))
        .await;
    assert_eq!(got["result"]["unattended_steps"], json!(null), "{got}");
    let key = "limits.unattended_steps";
    let reply = set(
        &mut client,
        "l1",
        "dev",
        json!([{"key": key, "value": 300}]),
    )
    .await;
    assert_eq!(reply["result"]["unattended_steps"], json!(300), "{reply}");
    assert_eq!(
        mine(&home, "dev").as_deref(),
        Some("[limits]\nunattended_steps = 300\n")
    );
    let got = client
        .call("g1", "preset.get", json!({"preset": "dev"}))
        .await;
    assert_eq!(got["result"]["unattended_steps"], json!(300), "{got}");
    let reply = set(&mut client, "l2", "dev", json!([{"key": key, "value": 0}])).await;
    assert_eq!(reason(&reply), Some("preset_invalid"), "{reply}");
    assert_eq!(
        mine(&home, "dev").as_deref(),
        Some("[limits]\nunattended_steps = 300\n"),
        "整条不收，什么都没写"
    );
    let reply = set(
        &mut client,
        "l3",
        "dev",
        json!([{"key": key, "unset": true}]),
    )
    .await;
    assert_eq!(reply["result"]["unattended_steps"], json!(null), "{reply}");
}
