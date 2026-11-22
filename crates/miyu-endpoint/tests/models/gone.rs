//! 供应商报得出自己的列表时照它为准（施工 FX-18）：从 `models.rs` 拆出来，用它的假核心和配置。

use serde_json::{Value, json};

use miyu_http::testkit::{Piece, Reply, Server};

use super::{config, data, list};
use crate::support::*;

/// 拉到了列表：照它为准（施工 FX-19），只写了资料的（`retired`、`x`）、用途里点名的（`vanished`）列表里没有都不列。没拉过
/// 列表的那一次照旧都列。
#[tokio::test]
async fn models_the_provider_no_longer_offers_are_left_out_even_in_use() {
    let page = r#"{"data":[{"id":"claude-sonnet-4-5"},{"id":"only-here"}]}"#;
    // 两份：头一次不带 refresh 的在后台拉掉一份，带 refresh 的拉另一份。
    let reply = || Reply::stream(vec![Piece::Bytes(page.as_bytes().to_vec())]);
    let server = Server::start(vec![reply(), reply()]).await;
    let home = Home::new();
    let text = config(&server.base_url).replace(
        "[models]\n",
        "[providers.newapi.models.retired]\nwindow = 8000\n\n[models]\nvision = \"newapi/vanished\"\n",
    );
    home.write("etc/config.toml", &text);
    let env = [("NEWAPI_KEY", "sk-relay")];
    let names = |answer: &Value| -> Vec<String> {
        answer["result"]["providers"][0]["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|model| model["model"].as_str().unwrap().to_string())
            .collect()
    };

    let before = list(&home, &env, data(), json!({"provider": "newapi"})).await;
    let listed = names(&before);
    assert!(
        listed.contains(&"retired".to_string()),
        "没有列表的照旧列：{listed:?}"
    );

    let after = list(
        &home,
        &env,
        data(),
        json!({"provider": "newapi", "refresh": true}),
    )
    .await;
    assert_eq!(names(&after), ["claude-sonnet-4-5", "only-here"]);
    let newapi = &after["result"]["providers"][0];
    assert_eq!(
        super::model(newapi, "claude-sonnet-4-5")["listed"],
        json!(["config", "provider"])
    );
}
