//! 更多描述能分行（施工 P-9 三补，`docs/blueprint/config.md` 的几行字 `lines`）：`config.set` 写带换行的值存得进、读回来一样；
//! 一行的文字照旧不收换行。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 连上一个照磁盘上的配置起来的核心，握过手。
async fn client(home: &Home) -> Client {
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    client
}

async fn set(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "config.set", params).await
}

#[tokio::test]
async fn the_about_text_keeps_its_line_breaks() {
    let home = Home::new();
    let mut client = client(&home).await;
    let reply = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [{"key": "profile.about", "value": "喜欢猫\n写 Rust"}]}),
    )
    .await;
    assert_eq!(
        reply["result"]["keys"]["profile.about"]["value"], "喜欢猫\n写 Rust",
        "{reply}"
    );
    let reply = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "changes": [{"key": "profile.name", "value": "小\n林"}]}),
    )
    .await;
    assert!(reply.get("error").is_some(), "称呼还是一行：{reply}");
}
