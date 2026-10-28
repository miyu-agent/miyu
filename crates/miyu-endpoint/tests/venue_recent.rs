//! `venue.session` 的 `recent`（施工 O-13 再补，接入通讯平台要的）：只给群、1 到 1000 条，写错的 `bad_params` 带 `data.field`
//! 是 `recent`；写对了的照常往下走（这里的连接不是平台桥，造群会话回 `needs_approval`（没批 `act_for_external`，施工 SU-4 中起））。

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 拒绝的原因码、`data.field`。
fn refused(reply: &Value) -> (&str, &str) {
    (
        reply["error"]["data"]["reason"]
            .as_str()
            .unwrap_or_default(),
        reply["error"]["data"]["field"].as_str().unwrap_or_default(),
    )
}

#[tokio::test]
async fn recent_is_for_groups_and_within_range() {
    let home = Home::new();
    let mut client = Client::connect(bound_core(&home, &Script::new([]), Catalog::default()));
    client.hello().await;
    for (params, want) in [
        (
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001", "recent": 5}),
            ("bad_params", "recent"),
        ),
        (
            json!({"venue": "qq:group:1", "kind": "group", "recent": 0}),
            ("bad_params", "recent"),
        ),
        (
            json!({"venue": "qq:group:1", "kind": "group", "recent": 1001}),
            ("bad_params", "recent"),
        ),
        (
            json!({"venue": "qq:group:1", "kind": "group", "recent": 1000}),
            ("needs_approval", ""),
        ),
    ] {
        let reply = client.call("v", "venue.session", params.clone()).await;
        assert_eq!(refused(&reply), want, "{params}：{reply}");
    }
}
