//! 照人的登录连核心、交接票（接入通讯平台要的，它的设计 01 第九节第 2、3 条）：真核心、真套接字。用一次性码设好密码拿到登录令牌，
//! `miyu_client` 的 `connect_login` 照它另开一条连接（不读本机令牌）；这条连接要一张交接票，换出一个登录令牌，再换一次不认。
//! 登录令牌不认的连不上；照用户名、密码连（`connect_password`）换到登录令牌。拒绝照 `Refused::reason` 认（施工 W-8 再补）。

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::support::{Home, within};
use miyu_client::open::Core;
use miyu_ipc::{connect_bare, connect_or_start};

/// 在裸连接上发几条请求，一条一条读回应。
async fn ask(connection: miyu_ipc::Connection, requests: &[Value]) -> Vec<Value> {
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read);
    let mut replies = Vec::new();
    for request in requests {
        write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        let mut line = String::new();
        within("回应", lines.read_line(&mut line))
            .await
            .expect("读得到");
        replies.push(serde_json::from_str(&line).expect("是 JSON"));
    }
    replies
}

#[tokio::test]
async fn a_login_connection_hands_off_a_ticket_once() {
    let home = Home::new();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起核心");
    let mut local = within("本机令牌连上", Core::connect_running(&home.root, "test"))
        .await
        .expect("连得上");
    let code = local
        .call("c", "account.setup_code", json!({}))
        .await
        .expect("要得到一次性码");
    let code = code["code"].as_str().expect("有码").to_string();
    let bare = connect_bare(&home.root).await.expect("连得上");
    let replies = ask(
        bare,
        &[
            json!({"jsonrpc": "2.0", "id": "h", "method": "hello", "params": {
                "protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"}, "code": code}}),
            json!({"jsonrpc": "2.0", "id": "s", "method": "account.setup", "params": {
                "username": "admin", "password": "correct horse battery"}}),
        ],
    )
    .await;
    let login = replies[1]["result"]["login"]["token"]
        .as_str()
        .unwrap_or_else(|| panic!("{replies:?}"))
        .to_string();

    let mut im = within(
        "登录令牌连上",
        Core::connect_login(&home.root, &login, "im"),
    )
    .await
    .expect("照登录令牌连得上");
    let ticket = im
        .call("t", "login.ticket", json!({}))
        .await
        .expect("人要得到票");
    let ticket = ticket["ticket"].as_str().expect("有票").to_string();
    let redeemed = im
        .call("r", "login.redeem", json!({"ticket": ticket}))
        .await
        .expect("换得到");
    let token = redeemed["login"]["token"].as_str().expect("换出登录令牌");
    assert_ne!(token, login, "换出的是新的一个");
    within(
        "换出的也连得上",
        Core::connect_login(&home.root, token, "im"),
    )
    .await
    .expect("换出的登录令牌认得");
    let again = im
        .call("r2", "login.redeem", json!({"ticket": ticket}))
        .await
        .err()
        .map(|refused| refused.reason);
    assert_eq!(again.as_deref(), Some("bad_ticket"), "一张票只换一次");
    let wrong = Core::connect_login(&home.root, "not-a-login", "im")
        .await
        .err()
        .map(|refused| refused.reason);
    assert_eq!(wrong.as_deref(), Some("bad_login"), "登录令牌不认的连不上");

    let (mut by_password, issued) = within(
        "用户名、密码连上",
        Core::connect_password(&home.root, "admin", "correct horse battery", "im"),
    )
    .await
    .expect("用户名、密码对");
    assert_eq!(issued.token.len(), 64, "换到一个登录令牌");
    assert!(!issued.expires.is_empty());
    assert_eq!(
        by_password.account(),
        redeemed["account"].as_str(),
        "握手回的账号存下来"
    );
    by_password
        .call("t2", "login.ticket", json!({}))
        .await
        .expect("这条连接是人");
    within(
        "换到的也连得上",
        Core::connect_login(&home.root, &issued.token, "im"),
    )
    .await
    .expect("认得");
    let bad = Core::connect_password(&home.root, "admin", "wrong password", "im")
        .await
        .err()
        .map(|refused| refused.reason);
    assert_eq!(bad.as_deref(), Some("bad_password"));
    drop(held);
}
