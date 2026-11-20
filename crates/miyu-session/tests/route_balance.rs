//! DeepSeek 官方的余额（施工 QU-2，`internal/blueprint/models.md`「怎么走」第十七条）：供应商的地址是官方的（没写，或者字面地址的
//! 主机是 api.deepseek.com）才查，GET 档案的地址、key 在 `Authorization` 里，读成余额；经中转站跑的、写了 catalog 指中转站的、
//! 地址是环境变量引用的一律不发，配置改了查的地址也不发；60 秒内再问不再发；401、500、乱码当没有、不报错。
//!
//! 查余额的地址指着本机的假服务器（档案或者配置改的），不打真网；说话的地址只拿来判是不是官方的，不发请求。

use std::sync::Arc;

use serde_json::json;

use crate::support::routing::resolved;
use miyu_config::secret::{Reference, Secret};
use miyu_http::testkit::{Reply, Server};
use miyu_http::{Proxy, fetcher};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::quota::{Balance, BalanceItem};
use miyu_session::{ModelData, Observed, quota};

/// 官方文档的样子。
const BALANCE: &str = r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"12.34","granted_balance":"2.00","topped_up_balance":"10.34"}]}"#;

fn json_reply(status: u16, body: &str) -> Reply {
    Reply::error(status, &[("Content-Type", "application/json")], body)
}

/// 档案：`deepseek` 照出厂的驱动、官方地址，查余额的地址是 `url`。能拉东西（不走代理）、没有目录。
fn data(url: &str) -> Arc<ModelData> {
    let profiles = Profiles::parse(&json!({"providers": {"deepseek": {
        "driver": "openai-chat",
        "base_url": "https://api.deepseek.com",
        "quota": {"url": url, "format": "deepseek-balance"}
    }}}))
    .expect("档案写法对");
    let data = ModelData::new(profiles, Vendors::default(), None)
        .with_fetcher(fetcher(Proxy::Off).expect("造得出客户端"));
    data.loaded(None, Observed::default());
    Arc::new(data)
}

fn secret(reference: &Reference) -> Option<Secret> {
    (*reference == Reference::Env("DS_KEY".to_string()))
        .then(|| Secret::new("sk-ds").expect("合写法"))
}

/// 供应商 `id` 照 `lines` 写，key 是 `{ env = "DS_KEY" }`，`models.chat` 指着它；问一次它的余额。
async fn ask(data: &ModelData, id: &str, lines: &str) -> Option<Balance> {
    let source = format!(
        "[providers.{id}]\nkey = {{ env = \"DS_KEY\" }}\n{lines}\n[models]\nchat = \"{id}/deepseek-v4.1-flash\"\n"
    );
    let values = resolved(&source).values();
    let reading = quota(data, &values, &secret, id).await?;
    assert!(reading.windows.is_empty(), "余额没有窗口");
    reading.balance
}

fn wanted() -> Balance {
    Balance {
        available: true,
        items: vec![BalanceItem {
            currency: "CNY".to_string(),
            total: "12.34".to_string(),
            granted: "2.00".to_string(),
            topped_up: "10.34".to_string(),
        }],
    }
}

#[tokio::test]
async fn the_official_provider_reads_its_balance_with_the_key_and_reuses_it_for_a_minute() {
    let server = Server::start(vec![json_reply(200, BALANCE), json_reply(200, BALANCE)]).await;
    let data = data(&format!("{}/user/balance", server.base_url));
    assert_eq!(ask(&data, "deepseek", "").await, Some(wanted()));
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, "GET");
    assert_eq!(received[0].path, "/v1/user/balance");
    assert_eq!(received[0].header("authorization"), Some("Bearer sk-ds"));
    assert_eq!(
        ask(&data, "deepseek", "").await,
        Some(wanted()),
        "60 秒内照上一次的"
    );
    assert_eq!(server.received().len(), 1, "60 秒内不再发");
}

#[tokio::test]
async fn a_literal_official_address_and_a_moved_balance_address_are_asked() {
    let server = Server::start(vec![json_reply(200, BALANCE)]).await;
    let data = data("http://127.0.0.1:9/never");
    let lines = format!(
        "base_url = \"https://api.deepseek.com/v1\"\n[providers.deepseek.quota]\nurl = \"{}/moved\"\n",
        server.base_url
    );
    assert_eq!(ask(&data, "deepseek", &lines).await, Some(wanted()));
    assert_eq!(server.received()[0].path, "/v1/moved");
}

#[tokio::test]
async fn a_relay_or_an_env_address_is_never_asked_even_with_a_moved_balance_address() {
    let server = Server::start(vec![json_reply(200, BALANCE)]).await;
    let moved = format!(
        "[providers.{{id}}.quota]\nurl = \"{}/moved\"\n",
        server.base_url
    );
    for (id, lines) in [
        (
            "deepseek",
            "base_url = \"https://relay.example.invalid/v1\"\n",
        ),
        (
            "deepseek",
            "base_url = \"https://relay.example.invalid/api.deepseek.com/v1\"\n",
        ),
        ("deepseek", "base_url = { env = \"DS_URL\" }\n"),
        (
            "relay",
            "catalog = \"deepseek\"\nbase_url = \"https://relay.example.invalid/v1\"\n",
        ),
        (
            "relay",
            "catalog = \"deepseek\"\nbase_url = { env = \"DS_URL\" }\n",
        ),
    ] {
        let data = data(&format!("{}/user/balance", server.base_url));
        assert_eq!(ask(&data, id, lines).await, None, "{id}：{lines}");
        let lines = format!("{lines}{}", moved.replace("{id}", id));
        assert_eq!(ask(&data, id, &lines).await, None, "{id}：{lines}");
    }
    assert!(
        server.received().is_empty(),
        "地址不是官方的，key 不发去查余额"
    );
}

#[tokio::test]
async fn a_refused_failed_or_garbled_reply_is_no_balance_and_not_asked_again_at_once() {
    for reply in [
        json_reply(401, r#"{"error":{"message":"Authentication Fails"}}"#),
        json_reply(500, "oops"),
        json_reply(200, "<html>not json</html>"),
        json_reply(
            200,
            r#"{"is_available":true,"balance_infos":[{"currency":"CNY"}]}"#,
        ),
    ] {
        let status = reply.status;
        let server = Server::start(vec![reply, json_reply(200, BALANCE)]).await;
        let data = data(&format!("{}/user/balance", server.base_url));
        assert_eq!(ask(&data, "deepseek", "").await, None, "{status}");
        assert_eq!(ask(&data, "deepseek", "").await, None, "{status}");
        assert_eq!(server.received().len(), 1, "{status}：查不到的也算一次");
    }
}
