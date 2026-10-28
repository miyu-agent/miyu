//! 设备令牌连上的设、改访问密码（施工 SU-1 补：网页从 `miyu web` 打开、局域网登录的都是设备令牌）：设过的要带对现在的，照来源
//! 限流；还没设的直接设；握手回应照实写设没设。

use std::sync::Arc;

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::login::*;
use crate::support::*;

/// 设备令牌连上的改访问密码（施工 SU-1 补：网页从 `miyu web` 打开、局域网登录的都是设备令牌）：要带对现在的 `old`，不带
/// `bad_params`、错的 `bad_password`，照来源限流同密码登录；改好了别的设备断开，这个连接换一个新的设备令牌、照旧用得了。握手
/// 回应照实写设过没有。
#[tokio::test]
async fn a_device_changes_the_password_with_the_old_one() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let token = set_up(&core, "correct horse").await;
    let mut other = remote(&core, "192.168.1.30").await;
    let reply = hello_with(&mut other, json!({"password": "correct horse"})).await;
    assert_eq!(reply["result"]["password_set"], true, "{reply}");
    let mut device = remote(&core, "192.168.1.20").await;
    let reply = hello_with(&mut device, json!({"login": token})).await;
    assert_eq!(reply["result"]["password_set"], true, "{reply}");
    let bare = device
        .call("s", "account.setup", json!({"password": "battery staple"}))
        .await;
    assert_eq!(why(&bare), "bad_params", "没带现在的：{bare}");
    let wrong = device
        .call(
            "s",
            "account.setup",
            json!({"password": "battery staple", "old": "wrong one"}),
        )
        .await;
    assert_eq!(why(&wrong), "bad_password", "{wrong}");
    let reply = device
        .call(
            "s",
            "account.setup",
            json!({"password": "battery staple", "old": "correct horse"}),
        )
        .await;
    let fresh = reply["result"]["login"]["token"]
        .as_str()
        .unwrap_or_else(|| panic!("换一个新的：{reply}"))
        .to_string();
    assert!(is_closed(&mut other).await, "别的设备断开");
    let still = device.call("l", "session.list", json!({})).await;
    assert!(still.get("result").is_some(), "这个连接照旧用得了：{still}");
    let mut stale = Client::connect(Arc::clone(&core));
    assert_eq!(
        why(&hello_with(&mut stale, json!({"login": token})).await),
        "bad_login"
    );
    let mut again = Client::connect(Arc::clone(&core));
    assert!(
        hello_with(&mut again, json!({"login": fresh}))
            .await
            .get("result")
            .is_some()
    );
    // 同一个来源错 5 次，第 6 次带对的也先拒、不改。
    let mut guessing = remote(&core, "192.168.1.40").await;
    hello_with(&mut guessing, json!({"login": fresh})).await;
    for n in 0..5 {
        let reply = guessing
            .call(
                "s",
                "account.setup",
                json!({"password": "stolen phone", "old": "wrong one"}),
            )
            .await;
        assert_eq!(why(&reply), "bad_password", "第 {n} 次：{reply}");
    }
    let held = guessing
        .call(
            "s",
            "account.setup",
            json!({"password": "stolen phone", "old": "battery staple"}),
        )
        .await;
    assert_eq!(why(&held), "login_throttled", "{held}");
    let mut fresh_login = Client::connect(Arc::clone(&core));
    assert!(
        hello_with(&mut fresh_login, json!({"password": "battery staple"}))
            .await
            .get("result")
            .is_some(),
        "没改成"
    );
}

/// 还没设访问密码时，交接票换来的设备令牌连上的（网页从 `miyu web` 打开）直接设，不用带 `old`；设好了这个连接换新的设备令牌。
#[tokio::test]
async fn a_ticket_device_sets_the_first_password() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut local = local(&core).await;
    let ticket = local.call("t", "login.ticket", json!({})).await;
    let value = ticket["result"]["ticket"]
        .as_str()
        .expect("有票")
        .to_string();
    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"ticket": value})).await;
    assert_eq!(shaken["result"]["password_set"], false, "{shaken}");
    let reply = browser
        .call("s", "account.setup", json!({"password": "correct horse"}))
        .await;
    assert!(reply["result"]["login"]["token"].is_string(), "{reply}");
    let mut phone = remote(&core, "192.168.1.20").await;
    let reply = hello_with(&mut phone, json!({"password": "correct horse"})).await;
    assert_eq!(reply["result"]["password_set"], true, "{reply}");
}
