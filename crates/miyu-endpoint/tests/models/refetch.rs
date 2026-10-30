//! 列整份的 `model.list` 自己在后台拉、变了推 `models.changed`（施工 V-26）：从 `models.rs` 拆出来，用它的假核心和配置。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_http::testkit::{Piece, Reply, Server};

use super::{config, core, data};
use crate::support::*;

/// 列整份的 `model.list` 每家最多一分钟在后台拉一次，拉到的变了推 `models.changed` 给订着配置的连接（施工 V-26）；一样的不推；
/// 只要几个模型的（底栏）有了列表、没旧过一天的不拉。
#[tokio::test]
async fn a_whole_list_refetches_once_a_minute_and_announces_a_change() {
    let page = |ids: &[&str]| {
        let data: Vec<Value> = ids.iter().map(|id| json!({"id": id})).collect();
        Reply::stream(vec![Piece::Bytes(
            json!({"data": data}).to_string().into_bytes(),
        )])
    };
    let server = Server::start(vec![page(&["a", "b"]), page(&["a", "b"])]).await;
    let home = Home::new();
    home.write("config.toml", &config(&server.base_url));
    let shared = data();
    let core = core(&home, &[("NEWAPI_KEY", "sk-relay")], Arc::clone(&shared));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let reply = client
        .call("s1", "subscribe", json!({"stream": "config"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    client
        .call("l1", "model.list", json!({"provider": "newapi"}))
        .await;
    let pushed = client.next().await.expect("十秒内推了");
    assert_eq!(
        pushed,
        json!({"jsonrpc": "2.0", "method": "models.changed", "params": {"providers": ["newapi"]}}),
        "头一次拉到"
    );
    // 一分钟里再列整份、只要几个：都不再拉。
    client
        .call("l2", "model.list", json!({"provider": "newapi"}))
        .await;
    client
        .call("l3", "model.list", json!({"models": ["newapi/a"]}))
        .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(server.received().len(), 1, "一分钟里只拉一次");
    // 拉回来一样的（人按刷新）：不推。
    let (pushes, _) = {
        client
            .line(
                &json!({"jsonrpc": "2.0", "id": "l4", "method": "model.list",
            "params": {"provider": "newapi", "refresh": true}})
                .to_string(),
            )
            .await;
        client.until_reply("l4").await
    };
    assert!(pushes.is_empty(), "一样的不推：{pushes:?}");
    assert_eq!(server.received().len(), 2, "人要的照拉");
}
