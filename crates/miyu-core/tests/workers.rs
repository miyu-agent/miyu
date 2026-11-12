//! 工作线程数这一项（施工 CPU-1，`docs/blueprint/core.md`「起来的先后」第 7 条）：`core.workers` 在核心登记的清单里、
//! `config.schema` 里，整数 0 到 64，默认 0（照核数），只在系统那一层，改了重启核心生效；写进去的回应也说要重启。
//! 取数和夹住见 `src/workers/tests.rs`。

use std::sync::Arc;

use serde_json::json;

use crate::support::{Head, Home};
use miyu_core::serve;
use miyu_session::testkit::Script;

/// 这一项的键。
const KEY: &str = "core.workers";

#[test]
fn the_worker_count_is_registered_for_the_system_layer_only() {
    let item = miyu_core::settings::items()
        .into_iter()
        .find(|item| item.key == KEY)
        .expect("登记了 core.workers");
    assert_eq!(item.default, Some(miyu_config::Value::Int(0)), "默认照核数");
    assert_eq!(item.layers, [miyu_config::Layer::System], "只在系统那一层");
    assert_eq!(item.applies, miyu_config::Applies::Restart, "重启核心生效");
    assert_eq!(item.applies.as_str(), "restart");
}

#[tokio::test]
async fn the_schema_and_a_set_say_it_waits_for_a_restart() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core_configured(Arc::new(Script::new([])), &opened.token);
    let running = tokio::spawn(serve(
        opened.listener,
        core,
        || None,
        std::future::pending(),
    ));
    let mut head = Head::connect(&home.root).await;
    let reply = head
        .call(
            "workers-5c1e9a73d2b4f806-1",
            "config.schema",
            json!({"keys": [KEY]}),
        )
        .await;
    let items = reply["result"]["items"].as_array().expect("有项");
    assert_eq!(items.len(), 1, "{reply}");
    let item = &items[0];
    assert_eq!(item["key"], KEY, "{item}");
    assert_eq!(item["type"], "int", "{item}");
    assert_eq!(item["default"], 0, "{item}");
    assert_eq!(
        (&item["min"], &item["max"]),
        (&json!(0), &json!(64)),
        "{item}"
    );
    assert_eq!(item["layers"], json!(["system"]), "{item}");
    assert_eq!(item["applies"], "restart", "{item}");
    assert_eq!(
        (&item["page"], &item["group"]),
        (&json!("advanced"), &json!("core")),
        "{item}"
    );
    let set = head
        .call(
            "workers-5c1e9a73d2b4f806-2",
            "config.set",
            json!({"layer": "system", "changes": [{"key": KEY, "value": 3}]}),
        )
        .await;
    assert_eq!(set["result"]["keys"][KEY]["applies"], "restart", "{set}");
    let refused = head
        .call(
            "workers-5c1e9a73d2b4f806-3",
            "config.set",
            json!({"layer": "system", "changes": [{"key": KEY, "value": 65}]}),
        )
        .await;
    assert!(refused.get("error").is_some(), "超过 64 的不收：{refused}");
    drop(head);
    running.abort();
}
