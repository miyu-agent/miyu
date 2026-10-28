//! 包推给人（施工 IM-1 中下二）：扩展调 `package.emit`，核心推 `package.event` 给订了这个包的连接；订了别的包的收不到。没装的包订不了，人调 `package.emit` 回 `not_an_extension`，事件名不对的 `bad_params`。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::*;

/// 等到 `client` 收到一条 `package.event`：别的跳过，最多 60 秒。
async fn event(client: &mut Client) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到 package.event"));
        if message["method"] == "package.event" {
            return message["params"].clone();
        }
    }
}

/// `client` 这时手里、接下来一小会儿到的推送里有没有 `package.event`。
async fn heard(client: &mut Client) -> bool {
    let mut any = false;
    while let Some(message) = client.next_within(Duration::from_millis(300)).await {
        any |= message["method"] == "package.event";
    }
    any
}

#[tokio::test]
async fn a_package_pushes_to_its_own_account_s_subscribers() {
    let home = Home::new();
    let program = Program::new();
    let go = home.work.join("go");
    let pause = format!("pause:{}", go.display());
    let (path, keep) = record(&home, "watch");
    install_providing(
        &home,
        "watch",
        &program.name(),
        "always",
        &steps(&[
            &keep,
            "hello",
            &pause,
            r#"ask:package.emit:{"event":"cards.changed","data":{"n":1}}"#,
            r#"ask:package.emit:{"event":"Bad Name"}"#,
            r#"ask:subscribe:{"stream":"package","package":"watch"}"#,
            "serve",
        ]),
    );
    install_providing(
        &home,
        "other",
        &program.name(),
        "always",
        &steps(&["hello", "serve"]),
    );
    let core = core_with_settings(&home, quick());

    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    let subscribed = owner
        .call(
            "p1",
            "subscribe",
            json!({"stream": "package", "package": "watch"}),
        )
        .await;
    assert_eq!(subscribed["result"], json!({}), "{subscribed}");
    let unknown = owner
        .call(
            "p2",
            "subscribe",
            json!({"stream": "package", "package": "nowhere"}),
        )
        .await;
    assert_eq!(
        unknown["error"]["data"]["reason"], "unknown_package",
        "{unknown}"
    );
    let mut others = Client::connect(Arc::clone(&core));
    others.hello().await;
    let other = others
        .call(
            "p3",
            "subscribe",
            json!({"stream": "package", "package": "other"}),
        )
        .await;
    assert_eq!(other["result"], json!({}), "{other}");
    let said = owner
        .call("e1", "package.emit", json!({"event": "x"}))
        .await;
    assert_eq!(
        said["error"]["data"]["reason"], "not_an_extension",
        "{said}"
    );

    std::fs::write(&go, "").expect("写得进");
    let pushed = event(&mut owner).await;
    assert_eq!(
        pushed,
        json!({"package": "watch", "event": "cards.changed", "data": {"n": 1}})
    );
    assert!(!heard(&mut others).await, "订了别的包的收不到");

    // 头一行是工作目录，接着握手和三步的回应。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 5 {
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let replies = read(&path);
    let replies: Vec<Value> = replies
        .lines()
        .skip(2)
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    assert_eq!(replies[0]["result"], json!({}), "{replies:?}");
    assert_eq!(
        replies[1]["error"]["data"]["reason"], "bad_params",
        "{replies:?}"
    );
    assert_eq!(
        replies[2]["error"]["data"]["reason"], "local_only",
        "扩展订不了：{replies:?}"
    );
    core.stop_extensions().await;
}
