//! 不列进选择的预设（施工 LF-5 上，`docs/blueprint/presets.md`「格式」、`preset.list`、`preset.get`）：真核心走一遍。
//! `preset.list`、`preset.get` 的一项都带 `hidden`，出厂四个都是 `false`；`preset.set` 写了 `hidden = true` 再读是 `true`，
//! 去掉以后回到 `false`；核心照旧列出它、照旧能拿它开会话（不列是头的事）。

use serde_json::{Value, json};

use crate::support::*;

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(crate::support::packaged::core(home));
    client.hello().await;
    client
}

/// `preset.list` 里每一项的编号和 `hidden`。
async fn listed(client: &mut Client) -> Vec<(String, Value)> {
    let listed = client.call("l", "preset.list", json!({})).await;
    listed["result"]["presets"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .map(|one| {
            (
                one["preset"].as_str().unwrap_or_default().to_string(),
                one["hidden"].clone(),
            )
        })
        .collect()
}

#[tokio::test]
async fn presets_carry_hidden_and_it_can_be_set_and_taken_away() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let factory = ["dev", "full", "minimal", "none"].map(|id| (id.to_string(), json!(false)));
    assert_eq!(listed(&mut client).await, factory, "出厂四个都是 false");
    for id in ["dev", "full", "minimal", "none"] {
        let got = client.call("g", "preset.get", json!({"preset": id})).await;
        assert_eq!(got["result"]["hidden"], json!(false), "{id}: {got}");
    }

    let set = client
        .call(
            "s",
            "preset.set",
            json!({"preset": "dev", "changes": [{"key": "preset.hidden", "value": true}]}),
        )
        .await;
    assert_eq!(set["result"]["hidden"], json!(true), "{set}");
    let got = client
        .call("g", "preset.get", json!({"preset": "dev"}))
        .await;
    assert_eq!(got["result"]["hidden"], json!(true), "{got}");
    let mut expected = factory.clone();
    expected[0].1 = json!(true);
    assert_eq!(
        listed(&mut client).await,
        expected,
        "照旧列出来，只是带上 true"
    );

    // 照旧能拿它开会话：核心不拦。
    let created = client
        .call("c", "session.create", json!({"cwd": "~", "preset": "dev"}))
        .await;
    assert!(created["result"]["session"].is_string(), "{created}");

    let wrong = client
        .call(
            "w",
            "preset.set",
            json!({"preset": "dev", "changes": [{"key": "preset.hidden", "value": "yes"}]}),
        )
        .await;
    assert_eq!(
        wrong["error"]["data"]["reason"], "preset_invalid",
        "{wrong}"
    );

    let back = client
        .call(
            "b",
            "preset.set",
            json!({"preset": "dev", "changes": [{"key": "preset.hidden", "unset": true}]}),
        )
        .await;
    assert_eq!(
        back["result"]["hidden"],
        json!(false),
        "去掉以后回到 false：{back}"
    );
    assert_eq!(listed(&mut client).await, factory);
}
