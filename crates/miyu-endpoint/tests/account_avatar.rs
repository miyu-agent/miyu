//! 账号头像（施工 AV-1，接入通讯平台设计 07 第 4.6 节第 8 样）：人换、删自己的头像，读得回；别人的只有管理员读得了；
//! 握手回应、`account.list` 带版本；对照的版本不对的 `avatar_conflict`；不是图、太大的照人格头像的规矩拒。

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::attach::png;
use crate::support::login::member_with_token;
use crate::support::*;
use miyu_session::testkit::Script;

/// 传一份内容，交回它的哈希。
async fn put(client: &mut Client, bytes: &[u8]) -> String {
    let reply = client
        .call(
            "p",
            "blob.put",
            json!({"data": STANDARD.encode(bytes), "name": "a.png"}),
        )
        .await;
    reply["result"]["blob"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_string()
}

/// 回应里拒绝的原因；没拒绝的是 `ok`。
fn why(reply: &Value) -> &str {
    reply["error"]["data"]["reason"].as_str().unwrap_or("ok")
}

#[tokio::test]
async fn a_person_sets_reads_and_unsets_their_own_avatar() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut admin = Client::connect(Arc::clone(&core));
    let hello = admin.hello().await;
    assert!(
        hello["result"].get("avatar").is_none(),
        "没有的不带：{hello}"
    );
    let (mut bob, token) = member_with_token(&core, "bob").await;

    // 换上：回版本，读得回原样。
    let picture = png(32, 32);
    let blob = put(&mut bob, &picture).await;
    let set = bob
        .call("a1", "account.set_avatar", json!({"blob": blob}))
        .await;
    let version = set["result"]["avatar"]
        .as_str()
        .unwrap_or_else(|| panic!("{set}"))
        .to_string();
    let read = bob.call("a2", "account.avatar", json!({})).await;
    assert_eq!(read["result"]["avatar"], version.as_str(), "{read}");
    assert_eq!(read["result"]["media_type"], "image/png");
    assert_eq!(read["result"]["data"], STANDARD.encode(&picture));

    // 别人的：成员读不了机主的，管理员读得了成员的。
    let refused = bob
        .call("a3", "account.avatar", json!({"account": "alice"}))
        .await;
    assert_eq!(why(&refused), "not_permitted", "{refused}");
    let seen = admin
        .call("a4", "account.avatar", json!({"account": "bob"}))
        .await;
    assert_eq!(seen["result"]["avatar"], version.as_str(), "{seen}");
    let listed = admin.call("a5", "account.list", json!({})).await;
    let row = listed["result"]["accounts"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["account"] == "bob"))
        .cloned()
        .unwrap_or_else(|| panic!("{listed}"));
    assert_eq!(row["avatar"], version.as_str(), "{row}");

    // 再连上：握手带版本。
    let mut again = Client::connect(Arc::clone(&core));
    let shook = crate::support::login::hello_with(&mut again, json!({"login": token})).await;
    assert_eq!(shook["result"]["avatar"], version.as_str(), "{shook}");

    // 对照的版本不对、不是图、太大的拒，什么都不动。
    let stale = bob
        .call(
            "a6",
            "account.set_avatar",
            json!({"unset": true, "expect": "0000000000000000"}),
        )
        .await;
    assert_eq!(why(&stale), "avatar_conflict", "{stale}");
    let text = put(&mut bob, b"not a picture").await;
    let bad = bob
        .call("a7", "account.set_avatar", json!({"blob": text}))
        .await;
    assert_eq!(why(&bad), "avatar_not_image", "{bad}");
    let huge = put(&mut bob, &png(1100, 10)).await;
    let big = bob
        .call("a8", "account.set_avatar", json!({"blob": huge}))
        .await;
    assert_eq!(why(&big), "avatar_too_big", "{big}");
    let read = bob.call("a9", "account.avatar", json!({})).await;
    assert_eq!(read["result"]["avatar"], version.as_str(), "没动：{read}");

    // 删掉：没有了。
    let unset = bob
        .call(
            "a10",
            "account.set_avatar",
            json!({"unset": true, "expect": version}),
        )
        .await;
    assert_eq!(unset["result"], json!({"avatar": null}), "{unset}");
    let read = bob.call("a11", "account.avatar", json!({})).await;
    assert_eq!(read["result"], Value::Null, "{read}");
}
