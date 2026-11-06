//! 扩展要交接票（施工 W-8 补、LP-1）：核心拉起的扩展只能要 `via` 是 `page` 的，要批过能力 `login.page`，没批的
//! `needs_approval`，别的 `via`、不写的照旧 `local_only`；它要的票只在本机的连接上换：本机的握手、`login.redeem` 换得到设备令牌，
//! 网页软件说过 `connection.remote` 的（局域网的、同机反向代理转来的回环地址）一律 `bad_ticket`，票当场作废。人要的 `open` 票照旧
//! 哪儿都能换（人要的 `page` 票施工 LP-2 起也只在本机换，见 `login_ticket.rs`）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;

use crate::support::extensions::{
    Program, approve, core, install_asking, quick, read, record, steps,
};
use crate::support::login::{hello_with, local, remote, set_up, why};
use crate::support::*;

/// 装上 `id`（声明 `capabilities`，`approved` 的算批过），拉起来照 `asks` 一步步问，等它记下握手和每一步的回应：交回核心和每一步
/// 的回应（不含握手）。
async fn asked(
    home: &Home,
    id: &str,
    capabilities: &[&str],
    asks: &[&str],
) -> (Arc<Core>, Vec<Value>) {
    let program = Program::new();
    let (path, keep) = record(home, id);
    let mut args = vec![keep.as_str(), "hello"];
    args.extend_from_slice(asks);
    args.push("wait");
    install_asking(
        home,
        id,
        &program.name(),
        "always",
        &steps(&args),
        capabilities,
    );
    let core = core(home, quick());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    // 第一行是工作目录，第二行是握手。
    while read(&path).lines().count() < 2 + asks.len() {
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let replies = read(&path)
        .lines()
        .skip(2)
        .map(|line| serde_json::from_str(line).expect("是 JSON"))
        .collect();
    core.stop_extensions().await;
    drop(program);
    (core, replies)
}

/// 回应里的票。
fn ticket(reply: &Value) -> String {
    reply["result"]["ticket"]
        .as_str()
        .unwrap_or_else(|| panic!("要得到票：{reply}"))
        .to_string()
}

const PAGE: &str = r#"ask:login.ticket:{"via":"page"}"#;

#[tokio::test]
async fn an_extension_without_login_page_gets_no_ticket() {
    let home = Home::new();
    let asks = [
        PAGE,
        "call:login.ticket",
        r#"ask:login.ticket:{"via":"open"}"#,
    ];
    let (_core, replies) = asked(&home, "asker", &[], &asks).await;
    assert_eq!(why(&replies[0]), "needs_approval", "{}", replies[0]);
    assert_eq!(
        replies[0]["error"]["data"]["capabilities"],
        json!(["login.page"]),
        "{}",
        replies[0]
    );
    assert_eq!(
        why(&replies[1]),
        "local_only",
        "不写 via 的照旧：{}",
        replies[1]
    );
    assert_eq!(
        why(&replies[2]),
        "local_only",
        "别的 via 照旧：{}",
        replies[2]
    );
}

#[tokio::test]
async fn an_approved_extension_gets_page_tickets_that_work_only_on_this_machine() {
    let home = Home::new();
    approve(&home, "pages", &["login.page"]);
    let asks = [
        PAGE,
        PAGE,
        PAGE,
        PAGE,
        PAGE,
        "call:login.ticket",
        r#"ask:login.ticket:{"via":"open"}"#,
    ];
    let (core, replies) = asked(&home, "pages", &["login.page"], &asks).await;
    assert!(
        replies[0]["result"]["expires"].is_string(),
        "{}",
        replies[0]
    );
    assert_eq!(
        why(&replies[5]),
        "local_only",
        "批过了也只认 page：{}",
        replies[5]
    );
    assert_eq!(why(&replies[6]), "local_only", "{}", replies[6]);

    // 本机的连接握手：换到设备令牌，令牌认得。
    let mut browser = Client::connect(Arc::clone(&core));
    let shaken = hello_with(&mut browser, json!({"ticket": ticket(&replies[0])})).await;
    let token = shaken["result"]["login"]["token"]
        .as_str()
        .unwrap_or_else(|| panic!("换到设备令牌：{shaken}"))
        .to_string();
    let mut later = Client::connect(Arc::clone(&core));
    let again = hello_with(&mut later, json!({"login": token})).await;
    assert!(again.get("result").is_some(), "{again}");

    // 本机的连接 `login.redeem` 也换得到。
    let mut owner = local(&core).await;
    let redeemed = owner
        .call("r", "login.redeem", json!({"ticket": ticket(&replies[1])}))
        .await;
    assert!(
        redeemed["result"]["login"]["token"].is_string(),
        "{redeemed}"
    );

    // 局域网来的握手换不到，票当场作废，回到本机也换不到。
    let lan = ticket(&replies[2]);
    let mut far = remote(&core, "192.168.1.30").await;
    let refused = hello_with(&mut far, json!({"ticket": lan})).await;
    assert_eq!(why(&refused), "bad_ticket", "{refused}");
    let mut back = Client::connect(Arc::clone(&core));
    let burnt = hello_with(&mut back, json!({"ticket": lan})).await;
    assert_eq!(why(&burnt), "bad_ticket", "作废了：{burnt}");

    // 同机反向代理转来的（网页软件照样说 `connection.remote`，地址是回环）也换不到。
    let mut proxied = remote(&core, "127.0.0.1").await;
    let refused = hello_with(&mut proxied, json!({"ticket": ticket(&replies[3])})).await;
    assert_eq!(why(&refused), "bad_ticket", "{refused}");

    // 远程登录着的连接 `login.redeem` 也换不到。
    set_up(&core, "correct horse").await;
    let mut signed = remote(&core, "::ffff:127.0.0.1").await;
    let shaken = hello_with(&mut signed, json!({"password": "correct horse"})).await;
    assert!(shaken.get("result").is_some(), "{shaken}");
    let refused = signed
        .call("r", "login.redeem", json!({"ticket": ticket(&replies[4])}))
        .await;
    assert_eq!(why(&refused), "bad_ticket", "{refused}");

    // 人要的 `open` 票照旧哪儿都能换。
    let person = ticket(
        &owner
            .call("t", "login.ticket", json!({"via": "open"}))
            .await,
    );
    let mut far = remote(&core, "192.168.1.30").await;
    let shaken = hello_with(&mut far, json!({"ticket": person})).await;
    assert!(shaken["result"]["login"]["token"].is_string(), "{shaken}");
}
