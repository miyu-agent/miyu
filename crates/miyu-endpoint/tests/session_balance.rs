//! 用量框里的余额（施工 QU-2，`session.quota` 的 `balance`）：真核心、真路由。会话用 DeepSeek 官方（配置没写地址，照档案是
//! 官方的），查余额的地址指着一台单独的假服务器：交 `balance`，金额、币种照官方原样，没有 `windows`。经中转站跑 DeepSeek 的
//! 会话回 `{}`，配置改了查余额的地址也不发请求。说话不发：官方地址的会话只问额度，不说话，不打真网。

use serde_json::{Value, json};

use miyu_http::testkit::{Reply, Server};

use crate::support::providers::{data, profiles, routed};
use crate::support::*;

/// 官方文档的样子。
const BALANCE: &str = r#"{"is_available":false,"balance_infos":[{"currency":"CNY","total_balance":"12.34","granted_balance":"2.00","topped_up_balance":"10.34"},{"currency":"USD","total_balance":"0.00","granted_balance":"0.00","topped_up_balance":"0.00"}]}"#;

fn json_reply(status: u16, body: &str) -> Reply {
    Reply::error(status, &[("Content-Type", "application/json")], body)
}

/// 界面中文，供应商 `id` 照 `lines` 写，key 照环境变量 `DS_KEY`，`models.chat` 指着它。
fn config(home: &Home, id: &str, lines: &str) {
    home.write(
        "etc/config.toml",
        &format!(
            "[ui]\nlanguage = \"zh\"\n\n[providers.{id}]\nkey = {{ env = \"DS_KEY\" }}\n{lines}\n[models]\nchat = \"{id}/deepseek-v4.1-flash\"\n"
        ),
    );
}

/// 档案：`deepseek` 官方的，查余额的地址是 `url`。
fn deepseek(url: &str) -> Value {
    json!({"deepseek": {
        "driver": "openai-chat",
        "base_url": "https://api.deepseek.com",
        "quota": {"url": url, "format": "deepseek-balance"}
    }})
}

/// 照家目录 `home` 起核心，开一个会话问一次 `session.quota`，交回结果。
async fn asked(home: &Home, url: &str) -> Value {
    let core = routed(home, &[("DS_KEY", "sk-ds")], data(profiles(deepseek(url))));
    let mut client = Client::connect(core);
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    let reply = client
        .call("q1", "session.quota", json!({"session": session}))
        .await;
    reply["result"].clone()
}

#[tokio::test]
async fn an_official_deepseek_session_gets_its_balance_as_written() {
    let balance = Server::start(vec![json_reply(200, BALANCE)]).await;
    let home = Home::new();
    config(&home, "deepseek", "");
    let result = asked(&home, &format!("{}/user/balance", balance.base_url)).await;
    assert_eq!(
        result,
        json!({"balance": {"available": false, "items": [
            {"currency": "CNY", "total": "12.34", "granted": "2.00", "topped_up": "10.34"},
            {"currency": "USD", "total": "0.00", "granted": "0.00", "topped_up": "0.00"}
        ]}}),
        "{result}"
    );
    let received = balance.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].header("authorization"), Some("Bearer sk-ds"));
}

#[tokio::test]
async fn a_deepseek_session_through_a_relay_has_no_balance_and_sends_nothing() {
    let balance = Server::start(vec![json_reply(200, BALANCE)]).await;
    let moved = format!(
        "\n[providers.relay.quota]\nurl = \"{}/moved\"\n",
        balance.base_url
    );
    for lines in [
        "catalog = \"deepseek\"\nbase_url = \"http://127.0.0.1:9/v1\"\n".to_string(),
        format!("catalog = \"deepseek\"\nbase_url = \"http://127.0.0.1:9/v1\"\n{moved}"),
    ] {
        let home = Home::new();
        config(&home, "relay", &lines);
        let result = asked(&home, &format!("{}/user/balance", balance.base_url)).await;
        assert_eq!(result, json!({}), "{lines}");
    }
    assert!(balance.received().is_empty(), "中转站的 key 不发去查余额");
}
