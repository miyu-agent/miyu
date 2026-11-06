//! 握手认交接票（施工 LT-1，接入通讯平台设计 07 第四节 4.6 第 4 样）：凭据多一种 `ticket`，待遇同 `password`：对上了当场
//! 作废，回应带新的设备令牌 `login`；用过、不认得的照 `login.redeem` 回 `bad_ticket`、断开；和别的凭据一起写的 `bad_params`。
//! 人要的 `page` 票只在本机换（施工 LP-2），别的照旧哪儿都能换。

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
    assert!(
        issued["result"].get("account").is_none(),
        "不带账号：{issued}"
    );
    let value = value(&issued);

    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"ticket": value})).await;
    assert!(shaken["result"].get("account").is_none(), "{shaken}");
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
    assert!(again.get("result").is_some(), "{again}");

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
        json!({"ticket": value, "password": "correct horse"}),
        json!({"ticket": value, "login": "a"}),
    ] {
        let mut client = Client::connect(Arc::clone(&core));
        let refused = hello_with(&mut client, credentials.clone()).await;
        assert_eq!(why(&refused), "bad_params", "{credentials}: {refused}");
    }
    // 上面一起写的没用掉这张票。
    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"ticket": value})).await;
    assert!(shaken["result"]["login"]["token"].is_string(), "{shaken}");
}

/// 要票时可以说一句是给谁的（施工 LT-2）：`via` 是 `page`（网页服务替本机页面要的）、`open`（`miyu web` 开网页要的），记进运行
/// 日志，不进回应（`page` 的只在本机换，施工 LP-2，下面）；不写的照旧。别的值、别的格回 `bad_params`。
#[tokio::test]
async fn a_ticket_may_say_what_it_is_for() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    for params in [json!({}), json!({"via": "page"}), json!({"via": "open"})] {
        let reply = owner.call("t", "login.ticket", params.clone()).await;
        assert!(reply["result"]["ticket"].is_string(), "{params}：{reply}");
        assert!(reply["result"].get("via").is_none(), "{reply}");
    }
    for params in [
        json!({"via": "elsewhere"}),
        json!({"via": 1}),
        json!({"other": true}),
    ] {
        let reply = owner.call("t", "login.ticket", params.clone()).await;
        assert_eq!(why(&reply), "bad_params", "{params}：{reply}");
    }
}

/// 人要的 `page` 票（施工 LP-2：网页软件 `/ticket` 出示本机令牌替本机页面要的）只在本机换：本机的握手、`login.redeem` 换得到；
/// 说过 `connection.remote` 的（局域网的、同机反向代理转来的回环地址）握手、`login.redeem` 都 `bad_ticket`，票当场作废，回本机也
/// 换不了。
#[tokio::test]
async fn a_persons_page_ticket_works_only_on_this_machine() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = local(&core).await;
    let page = json!({"via": "page"});
    let mut asked = Vec::new();
    for _ in 0..5 {
        asked.push(value(&owner.call("t", "login.ticket", page.clone()).await));
    }

    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"ticket": asked[0]})).await;
    assert!(shaken["result"]["login"]["token"].is_string(), "{shaken}");
    let redeemed = owner
        .call("r", "login.redeem", json!({"ticket": asked[1]}))
        .await;
    assert!(
        redeemed["result"]["login"]["token"].is_string(),
        "{redeemed}"
    );

    let mut far = remote(&core, "192.168.1.30").await;
    let refused = hello_with(&mut far, json!({"ticket": asked[2]})).await;
    assert_eq!(why(&refused), "bad_ticket", "{refused}");
    let mut back = Client::connect(Arc::clone(&core));
    let burnt = hello_with(&mut back, json!({"ticket": asked[2]})).await;
    assert_eq!(why(&burnt), "bad_ticket", "作废了：{burnt}");

    let mut proxied = remote(&core, "127.0.0.1").await;
    let refused = hello_with(&mut proxied, json!({"ticket": asked[3]})).await;
    assert_eq!(why(&refused), "bad_ticket", "同机反向代理转来的：{refused}");
    let again = owner
        .call("r", "login.redeem", json!({"ticket": asked[3]}))
        .await;
    assert_eq!(why(&again), "bad_ticket", "作废了：{again}");

    set_up(&core, "correct horse").await;
    let mut signed = remote(&core, "::ffff:127.0.0.1").await;
    let shaken = hello_with(&mut signed, json!({"password": "correct horse"})).await;
    assert!(shaken.get("result").is_some(), "{shaken}");
    let refused = signed
        .call("r", "login.redeem", json!({"ticket": asked[4]}))
        .await;
    assert_eq!(why(&refused), "bad_ticket", "{refused}");
}

/// 人要的别的票（`open`、不写 `via` 的）照旧哪儿都能换（施工 LP-2 只动 `page`）：远程的握手、`login.redeem` 都换得到。
#[tokio::test]
async fn a_persons_other_tickets_still_work_anywhere() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    set_up(&core, "correct horse").await;
    let mut owner = local(&core).await;
    for params in [json!({"via": "open"}), json!({})] {
        let one = value(&owner.call("t", "login.ticket", params.clone()).await);
        let mut proxied = remote(&core, "127.0.0.1").await;
        let shaken = hello_with(&mut proxied, json!({"ticket": one})).await;
        assert!(
            shaken["result"]["login"]["token"].is_string(),
            "{params}：{shaken}"
        );
        let two = value(&owner.call("t", "login.ticket", params.clone()).await);
        let mut signed = remote(&core, "192.168.1.30").await;
        let shaken = hello_with(&mut signed, json!({"password": "correct horse"})).await;
        assert!(shaken.get("result").is_some(), "{shaken}");
        let redeemed = signed
            .call("r", "login.redeem", json!({"ticket": two}))
            .await;
        assert!(
            redeemed["result"]["login"]["token"].is_string(),
            "{params}：{redeemed}"
        );
    }
}
