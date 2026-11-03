//! 谁来发后台通知（施工 TR-1，`docs/reference/connection.md`「`notices.claim`」）：托盘这样的头调 `notices.claim` 说「后台通知我来
//! 发」，核心照连接记、断了就忘。订阅 `notices` 的回应带这一刻有没有人认领（`claimed: true`，没有的不写），有没有变了推
//! `notices.claimed {claimed}`；两个认领的走了一个照样算有。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 读推送，直到 `notices.claimed`，交回它的 `claimed`。
async fn until_claimed(watcher: &mut Client) -> Value {
    loop {
        let next = watcher.next().await.expect("推了");
        if next["method"] == "notices.claimed" {
            return next["params"]["claimed"].clone();
        }
    }
}

/// 订阅 `notices`，交回回应。
async fn subscribe(client: &mut Client, id: &str) -> Value {
    let reply = client
        .call(id, "subscribe", json!({"stream": "notices"}))
        .await;
    reply["result"].clone()
}

#[tokio::test]
async fn a_claim_is_pushed_and_released_when_the_claimer_leaves() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut web = Client::connect(core.clone());
    web.hello().await;
    assert_eq!(subscribe(&mut web, "s1").await, json!({}), "没人认领不写");
    let mut tray = Client::connect(core.clone());
    tray.hello().await;
    let reply = tray.call("c1", "notices.claim", json!({})).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(until_claimed(&mut web).await, json!(true));
    // 后订阅的从回应里就知道。
    let mut late = Client::connect(core.clone());
    late.hello().await;
    assert_eq!(subscribe(&mut late, "s2").await, json!({"claimed": true}));
    // 同一个连接再认领一遍：没变，不推。
    tray.call("c2", "notices.claim", json!({})).await;
    drop(tray);
    assert_eq!(until_claimed(&mut web).await, json!(false), "连接断了就放");
    assert_eq!(subscribe(&mut late, "s3").await, json!({}));
}

#[tokio::test]
async fn two_claimers_one_leaves_still_claimed() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut web = Client::connect(core.clone());
    web.hello().await;
    subscribe(&mut web, "s1").await;
    let mut first = Client::connect(core.clone());
    first.hello().await;
    let mut second = Client::connect(core.clone());
    second.hello().await;
    first.call("c1", "notices.claim", json!({})).await;
    assert_eq!(until_claimed(&mut web).await, json!(true));
    second.call("c2", "notices.claim", json!({})).await;
    drop(first);
    // 还有一个认领着：不推，回应照样写着。
    assert!(
        web.next_within(Duration::from_millis(300)).await.is_none(),
        "还有人认领，不该推"
    );
    assert_eq!(subscribe(&mut web, "s2").await, json!({"claimed": true}));
    drop(second);
    assert_eq!(until_claimed(&mut web).await, json!(false));
}

#[tokio::test]
async fn a_claim_takes_no_params() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut tray = Client::connect(core);
    tray.hello().await;
    let bad = tray
        .call("c1", "notices.claim", json!({"session": "x"}))
        .await;
    assert_eq!(reason(&bad), Some("bad_params"), "{bad}");
}
