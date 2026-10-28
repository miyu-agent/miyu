//! 第一次引导走过了（施工 8-11 四补，`docs/blueprint/config.md`「M8 的配置项」）：`ui.welcomed` 写在系统配置里，
//! 设置页不画（Schema 里带 `hidden`），头走完引导写 `true`、经 `config.get` 读。

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::*;

#[tokio::test]
async fn the_welcome_flag_is_hidden_and_written_by_the_heads() {
    let home = Home::new();
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    let schema = client
        .call("s1", "config.schema", json!({"keys": ["ui.welcomed"]}))
        .await;
    let item = &schema["result"]["items"][0];
    assert_eq!(
        (
            &item["key"],
            &item["type"],
            &item["default"],
            &item["layers"],
            &item["hidden"],
            &item["name"]
        ),
        (
            &json!("ui.welcomed"),
            &json!("bool"),
            &json!(false),
            &json!(["system"]),
            &json!(true),
            &json!("第一次引导走过了")
        ),
        "{schema}"
    );
    let before = client.call("g1", "config.get", json!({})).await;
    assert!(
        before["result"]["items"]
            .get("ui.welcomed")
            .is_none_or(|item| item["value"] == false),
        "不写是假：{before}"
    );

    let written = client
        .call(
            "w1",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "ui.welcomed", "value": true}]}),
        )
        .await;
    assert!(written.get("error").is_none(), "{written}");
    let after = client.call("g2", "config.get", json!({})).await;
    assert_eq!(
        after["result"]["items"]["ui.welcomed"]["value"], true,
        "{after}"
    );
    let system =
        std::fs::read_to_string(home.root.path().join("config.toml")).expect("写进了系统配置");
    assert!(system.contains("[ui]\nwelcomed = true\n"), "{system}");
}
