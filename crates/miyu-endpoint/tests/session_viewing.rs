//! 哪个头开着哪个会话（施工 HV-1，`docs/reference/sessions.md`「`session.viewing`」）：一个头说开着某个会话，订阅列表的另一个头
//! 收到那一项多 `viewed: true`，`session.list` 也带；改成没开、连接断了，再推一次、`viewed` 没了。写法不对的 `bad_params`，没有
//! 这个会话的 `session_not_found`。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 读推送，直到会话 `session` 的那一项合 `wanted`，交回它。
async fn until_entry(
    watcher: &mut Client,
    session: &str,
    wanted: impl Fn(&Value) -> bool,
) -> Value {
    loop {
        let next = watcher.next().await.expect("十秒内推了");
        if next["method"] != "sessions.changed" || next["params"]["session"] != json!(session) {
            continue;
        }
        let entry = next["params"]["entry"].clone();
        if wanted(&entry) {
            return entry;
        }
    }
}

#[tokio::test]
async fn a_session_open_in_a_head_is_listed_as_viewed_until_it_leaves() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Says("好。")]));
    let mut watcher = Client::connect(core.clone());
    watcher.hello().await;
    let mut head = Client::connect(core.clone());
    head.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = head.create("c1", &work).await;
    watcher
        .call("w1", "subscribe", json!({"stream": "sessions"}))
        .await;
    let viewed = |entry: &Value| entry["viewed"] == json!(true);
    let reply = head
        .call("v1", "session.viewing", json!({"session": session}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    until_entry(&mut watcher, &session, viewed).await;
    let listed = watcher.call("l1", "session.list", json!({})).await;
    let item = listed["result"]["sessions"]
        .as_array()
        .expect("有列表")
        .iter()
        .find(|item| item["session"] == json!(session))
        .cloned()
        .expect("列着它");
    assert_eq!(item["viewed"], json!(true), "{item}");
    // 改成哪个都没开：推一次，`viewed` 不写了。
    head.call("v2", "session.viewing", json!({"session": null}))
        .await;
    let entry = until_entry(&mut watcher, &session, |entry| {
        entry.get("viewed").is_none()
    })
    .await;
    assert_eq!(entry["session"], json!(session));
    // 再开着它，连接断了：照样推一次。
    head.call("v3", "session.viewing", json!({"session": session}))
        .await;
    until_entry(&mut watcher, &session, viewed).await;
    drop(head);
    until_entry(&mut watcher, &session, |entry| {
        entry.get("viewed").is_none()
    })
    .await;
}

#[tokio::test]
async fn a_bad_or_unknown_session_is_refused() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut head = Client::connect(core);
    head.hello().await;
    let bad = head
        .call("v1", "session.viewing", json!({"session": "not-an-id"}))
        .await;
    assert_eq!(reason(&bad), Some("bad_params"), "{bad}");
    let unknown = head
        .call(
            "v2",
            "session.viewing",
            json!({"session": "01a19a00-0000-7000-8000-000000000001"}),
        )
        .await;
    assert_eq!(reason(&unknown), Some("session_not_found"), "{unknown}");
}
