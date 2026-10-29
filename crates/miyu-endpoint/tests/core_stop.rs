//! 头叫核心停下（施工 DT-4 下，`docs/blueprint/protocol.md` 方法表的 `core.stop`、`core.md`「停下」）：只给出示本机令牌连上的，
//! 回应 `{}` 写出去以后核心才收到叫停；设备令牌连上的回 `local_only`，远程的回 `local_machine_only`，核心照旧在跑。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::login::*;
use crate::support::*;

/// 这么久还没叫停，就是没叫停。
const QUIET: Duration = Duration::from_millis(300);

/// 叫停以后多久该到：到不了就是没叫停。
const SOON: Duration = Duration::from_secs(5);

#[tokio::test]
async fn the_local_token_asks_the_core_to_stop_after_the_reply() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let activity = core.activity();
    let mut head = local(&core).await;
    let reply = head.call("s", "core.stop", json!({})).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    tokio::time::timeout(SOON, activity.stop_requested())
        .await
        .expect("回应以后叫停了");
}

#[tokio::test]
async fn devices_and_remote_heads_cannot_and_the_core_keeps_running() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let token = set_up(&core, "correct horse").await;
    let mut device = Client::connect(Arc::clone(&core));
    hello_with(&mut device, json!({"login": token})).await;
    let refused = device.call("s", "core.stop", json!({})).await;
    assert_eq!(why(&refused), "local_only", "{refused}");
    let mut far = remote(&core, "203.0.113.9").await;
    hello_with(&mut far, json!({"password": "correct horse"})).await;
    let refused = far.call("s", "core.stop", json!({})).await;
    assert_eq!(why(&refused), "local_machine_only", "{refused}");
    assert!(
        tokio::time::timeout(QUIET, core.activity().stop_requested())
            .await
            .is_err(),
        "没叫停"
    );
    let listed = device.call("l", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "照旧能用：{listed}");
}
