//! 定死的预设（施工 FP-1，2026-11-28 项目主人定）：全部功能、无功能不许改，列表里置顶，`fixed: true`；改、删都拒
//! `preset_fixed`，什么都不写、不删；家目录那一层以前写的同名改动不再读，照出厂的算，文件留着。真核心走一遍。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::*;
use miyu_endpoint::Core;
use miyu_session::testkit::Script;

fn core(home: &Home) -> Arc<Core> {
    home.core(&Script::new([]))
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(core(home));
    client.hello().await;
    client
}

/// 家目录那一层的文件，没有的是空的。
fn mine(home: &Home, id: &str) -> Option<String> {
    std::fs::read_to_string(home.root.path().join(format!("root/presets/{id}.toml"))).ok()
}

/// 以前（施工 P-17 时）机主给无功能写过的改动：换了名字、开了记忆。
const OLD_NONE: &str = "[preset]\nname = \"我的无功能\"\n\n[features]\nmemory = true\n";

#[tokio::test]
async fn the_fixed_ones_come_first_and_say_so() {
    let home = Home::new();
    // 你那一层的预设照编号排在定死的后面：`a-mine` 照编号本该排第一。
    home.write("root/presets/a-mine.toml", "[preset]\nname = \"A\"\n");
    let mut client = connected(&home).await;
    let listed = client.call("l1", "preset.list", json!({})).await;
    let presets = listed["result"]["presets"].as_array().expect("有列表");
    let ids: Vec<&str> = presets
        .iter()
        .filter_map(|one| one["preset"].as_str())
        .collect();
    assert_eq!(
        ids,
        ["full", "none", "a-mine", "dev", "minimal"],
        "全部功能、无功能置顶，其余照编号：{listed}"
    );
    for one in presets {
        let fixed = matches!(one["preset"].as_str(), Some("full" | "none"));
        assert_eq!(
            one.get("fixed"),
            fixed.then_some(&json!(true)),
            "只有定死的带 fixed：{one}"
        );
    }
    for (n, (id, fixed)) in [("full", true), ("none", true), ("dev", false)]
        .into_iter()
        .enumerate()
    {
        let got = client
            .call(&format!("g{n}"), "preset.get", json!({"preset": id}))
            .await;
        assert_eq!(
            got["result"].get("fixed"),
            fixed.then_some(&json!(true)),
            "{id}：{got}"
        );
        if fixed {
            assert_eq!(got["result"]["remove"], Value::Null, "{id} 没什么可删");
        }
    }
}

#[tokio::test]
async fn writing_a_fixed_one_is_refused_and_touches_nothing() {
    let home = Home::new();
    home.write("root/presets/none.toml", OLD_NONE);
    let mut client = connected(&home).await;
    let change = json!([{"key": "preset.name", "value": "改个名"}]);
    for (n, id) in ["full", "none"].into_iter().enumerate() {
        let set = client
            .call(
                &format!("s{n}"),
                "preset.set",
                json!({"preset": id, "changes": change}),
            )
            .await;
        assert_eq!(reason(&set), Some("preset_fixed"), "{id}：{set}");
        let deleted = client
            .call(&format!("d{n}"), "preset.delete", json!({"preset": id}))
            .await;
        assert_eq!(reason(&deleted), Some("preset_fixed"), "{id}：{deleted}");
    }
    assert_eq!(mine(&home, "full"), None, "全部功能什么都没写");
    assert_eq!(
        mine(&home, "none").as_deref(),
        Some(OLD_NONE),
        "以前写的改动是人的数据：不改、不删"
    );
    // 带 `new` 的照旧先说编号已经有了。
    let new = client
        .call(
            "n1",
            "preset.set",
            json!({"preset": "full", "new": true, "changes": change}),
        )
        .await;
    assert_eq!(reason(&new), Some("preset_exists"), "{new}");
    // 别的出厂预设照旧能改。
    let dev = client
        .call(
            "s9",
            "preset.set",
            json!({"preset": "dev", "changes": change}),
        )
        .await;
    assert_eq!(dev["result"]["name"], "改个名", "{dev}");
}

#[tokio::test]
async fn an_old_change_to_a_fixed_one_in_your_layer_is_ignored() {
    let home = Home::new();
    home.write("root/presets/none.toml", OLD_NONE);
    let mut client = connected(&home).await;
    let got = client
        .call("g1", "preset.get", json!({"preset": "none"}))
        .await;
    assert_eq!(got["result"]["name"], "无功能", "照出厂的名字：{got}");
    let memory = got["result"]["features"]
        .as_array()
        .expect("有功能")
        .iter()
        .find(|one| one["id"] == "memory")
        .map(|one| one["on"].clone());
    assert_eq!(memory, Some(json!(false)), "照出厂的全关：{got}");
    let listed = client.call("l1", "preset.list", json!({})).await;
    let none = listed["result"]["presets"]
        .as_array()
        .expect("有列表")
        .iter()
        .find(|one| one["preset"] == "none")
        .cloned()
        .expect("列着无功能");
    assert_eq!(none["name"], "无功能", "{none}");
    assert_eq!(mine(&home, "none").as_deref(), Some(OLD_NONE), "文件留着");
}
