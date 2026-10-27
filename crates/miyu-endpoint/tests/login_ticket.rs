//! 握手认交接票（施工 LT-1，接入通讯平台设计 07 第四节 4.6 第 4 样）：凭据多一种 `ticket`，待遇同 `password`：对上了当场
//! 作废，回应带新的登录令牌 `login`，连接就是票的那个账号；用过、不认得的照 `login.redeem` 回 `bad_ticket`、断开；和别的凭据
//! 一起写的 `bad_params`。`login.ticket` 的回应带 `account`（票是谁的）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::login::*;
use crate::support::*;

/// 在 `client` 上要一张票：交回整个回应。
async fn ticket(client: &mut Client) -> Value {
    let reply = client.call("t", "login.ticket", json!({})).await;
    assert!(reply.get("result").is_some(), "要得到票：{reply}");
    reply
}

fn value(reply: &Value) -> String {
    reply["result"]["ticket"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_string()
}

#[tokio::test]
async fn a_ticket_logs_a_connection_in_once() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let issued = ticket(&mut owner).await;
    assert_eq!(issued["result"]["account"], "alice", "票是谁的：{issued}");
    let value = value(&issued);

    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"ticket": value})).await;
    assert_eq!(shaken["result"]["account"], "alice", "{shaken}");
    assert_eq!(shaken["result"]["admin"], true, "{shaken}");
    let token = shaken["result"]["login"]["token"]
        .as_str()
        .unwrap_or_else(|| panic!("回应带新的登录令牌：{shaken}"))
        .to_string();
    assert!(shaken["result"]["login"]["expires"].is_string(), "{shaken}");
    let asked = browser.call("w", "session.list", json!({})).await;
    assert!(asked.get("result").is_some(), "连上了就能用：{asked}");

    // 换出的登录令牌认得。
    let mut later = Client::connect(Arc::clone(&core));
    let again = hello_with(&mut later, json!({"login": token})).await;
    assert_eq!(again["result"]["account"], "alice", "{again}");

    // 同一张票只能用一次。
    let mut twice = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut twice, json!({"ticket": value})).await;
    assert_eq!(why(&refused), "bad_ticket", "{refused}");
    assert!(is_closed(&mut twice).await, "回完断开");
    // 用过的也换不出登录令牌。
    let redeemed = owner
        .call("r", "login.redeem", json!({"ticket": value}))
        .await;
    assert_eq!(why(&redeemed), "bad_ticket", "{redeemed}");
}

#[tokio::test]
async fn someone_else_s_ticket_logs_in_as_them() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut bob = member(&core, "bob").await;
    let issued = ticket(&mut bob).await;
    let whose = issued["result"]["account"]
        .as_str()
        .unwrap_or_else(|| panic!("{issued}"))
        .to_string();
    assert_ne!(whose, "alice", "是成员自己的账号：{issued}");

    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"ticket": value(&issued)})).await;
    assert_eq!(shaken["result"]["account"], whose.as_str(), "{shaken}");
    assert_eq!(
        shaken["result"]["admin"], false,
        "成员的票换不成管理员：{shaken}"
    );
}

#[tokio::test]
async fn a_bad_ticket_or_another_credential_with_it_is_refused() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut unknown = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut unknown, json!({"ticket": "0".repeat(64)})).await;
    assert_eq!(why(&refused), "bad_ticket", "{refused}");
    assert!(is_closed(&mut unknown).await, "回完断开");

    let mut owner = local(&core).await;
    let value = value(&ticket(&mut owner).await);
    for credentials in [
        json!({"ticket": value, "token": TOKEN}),
        json!({"ticket": value, "user": "shorin", "password": "correct horse"}),
        json!({"ticket": value, "login": "a"}),
    ] {
        let mut client = Client::connect(Arc::clone(&core));
        let refused = hello_with(&mut client, credentials.clone()).await;
        assert_eq!(why(&refused), "bad_params", "{credentials}: {refused}");
    }
    // 上面一起写的没用掉这张票。
    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"ticket": value})).await;
    assert_eq!(shaken["result"]["account"], "alice", "{shaken}");
}
