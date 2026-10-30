//! 这台机器上用不上的配置项不画（施工 UH-1）：测试的核心没装终端界面、也没装别的界面，终端集成会话那几项、默认界面在
//! `config.schema` 里标 `hidden`；别的照常。值照旧查得到。

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::*;

#[tokio::test]
async fn items_for_a_head_that_is_not_installed_are_hidden() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client
        .call(
            "c1",
            "config.schema",
            json!({"keys": ["terminal.persona", "terminal.idle_new_after", "ui.head", "ui.language"]}),
        )
        .await;
    let items = reply["result"]["items"].as_array().expect("有 items");
    let hidden = |key: &str| {
        items
            .iter()
            .find(|item| item["key"] == key)
            .unwrap_or_else(|| panic!("有 {key}：{reply}"))["hidden"]
            .clone()
    };
    assert_eq!(hidden("terminal.persona"), json!(true), "没装终端界面");
    assert_eq!(hidden("terminal.idle_new_after"), json!(true));
    assert_eq!(hidden("ui.head"), json!(true), "没得选");
    assert!(hidden("ui.language").is_null(), "别的照常画");
    let got = client
        .call("c2", "config.get", json!({"keys": ["ui.head"]}))
        .await;
    assert!(
        got["result"]["items"]["ui.head"].is_object(),
        "照旧查得到：{got}"
    );
}
