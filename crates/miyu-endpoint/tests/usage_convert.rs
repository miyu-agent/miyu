//! 金额换算（施工 UC-1，`docs/blueprint/models.md`「协议」`usage.query`）：人民币、美元两种价格各调一次，`usage.currency` 是人民币，
//! 汇率从假的汇率服务器取：`converted` 是换算好的一个数，`amounts` 原样；再查不再取；主源出错照兜底源、回应带 `rates`；
//! 取不到汇率的只换得了人民币本身那一笔、美元那一笔在 `rest`。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::runtime::Handle;

use crate::support::providers::{data, profiles, routed};
use crate::support::*;
use miyu_endpoint::Core;
use miyu_http::testkit::{Reply, Server};
use miyu_http::{Proxy, client};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_session::rates::HttpRates;

/// 一家 `a`：`a/m` 每百万输入 1、输出 2 人民币，`a/u` 每百万输入 1、输出 1 美元；`usage.currency` 是人民币。
fn config(server: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nlocal = false\n\n[providers.a.models.m.price]\ninput = 1.0\noutput = 2.0\ncurrency = \"CNY\"\n\n[providers.a.models.u.price]\ninput = 1.0\noutput = 1.0\n\n[models]\nchat = \"a/m\"\n\n[usage]\ncurrency = \"CNY\"\n",
        server.base_url
    )
}

/// 人民币那一次、美元那一次的金额（假服务器报 12 输入、3 输出）。
const CNY: f64 = (12.0 + 3.0 * 2.0) / 1e6;
const USD: f64 = (12.0 + 3.0) / 1e6;

/// 今天（UTC）写成 `2026-10-24`。
fn today() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("在 1970 年以后")
        .as_millis();
    Timestamp::from_unix_millis(i64::try_from(millis).expect("放得下"))
        .expect("合写法")
        .local_date(UtcOffset::UTC)
}

fn json_reply(body: &str) -> Reply {
    Reply::error(200, &[("Content-Type", "application/json")], body)
}

/// 起核心、装上对着 `rates` 的取汇率，调 `a/m`、`a/u` 各一次，交回连着的头。
async fn spent(home: &Home, rates: &Server) -> (Arc<Core>, Client) {
    let model = Server::start(vec![
        crate::support::providers::said("a"),
        crate::support::providers::said("b"),
    ])
    .await;
    home.write("config.toml", &config(&model));
    let core = routed(home, &[], data(profiles(json!({}))));
    let base = rates.base_url.trim_end_matches("/v1");
    core.install_rates(Arc::new(HttpRates::at(
        client(Proxy::Off).expect("造得出"),
        Handle::current(),
        base,
        base,
    )));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    for (id, model) in [("c1", "a/m"), ("c2", "a/u")] {
        let asked = json!({"model": model, "purpose": "platform", "messages": [{"role": "user", "text": "hi"}]});
        let reply = client.call(id, "model.call", asked).await;
        assert!(reply.get("result").is_some(), "{reply}");
    }
    // 留着假模型服务器到调完。
    drop(model);
    (core, client)
}

fn amount(value: &Value) -> f64 {
    value.as_f64().unwrap_or_else(|| panic!("是数：{value}"))
}

#[tokio::test]
async fn amounts_are_converted_at_the_days_rate_and_the_rate_is_kept() {
    let rates = Server::start(vec![json_reply(&format!(
        r#"[{{"date":"{}","base":"USD","quote":"CNY","rate":7.0}}]"#,
        today()
    ))])
    .await;
    let home = Home::new();
    let (_core, mut client) = spent(&home, &rates).await;
    let reply = client.call("q1", "usage.query", json!({})).await;
    let row = &reply["result"]["rows"][0];
    let converted = &row["converted"];
    assert_eq!(converted["currency"], "CNY", "{reply}");
    assert!(
        (amount(&converted["amount"]) - (CNY + USD * 7.0)).abs() < 1e-12,
        "人民币那一笔原样、美元那一笔乘 7：{reply}"
    );
    assert!(converted.get("rest").is_none(), "都换成了：{reply}");
    assert_eq!(
        row["amounts"].as_array().map(Vec::len),
        Some(2),
        "amounts 原样：{reply}"
    );
    assert!(
        reply["result"].get("rates").is_none(),
        "只用了主源，不写来源"
    );
    let again = client.call("q2", "usage.query", json!({})).await;
    assert_eq!(again["result"]["rows"][0]["converted"], *converted);
    let received = rates.received();
    assert_eq!(received.len(), 1, "再查不再取");
    assert!(
        received[0].path.contains("quotes=CNY&"),
        "只问人民币，美元不用问：{}",
        received[0].path
    );
}

#[tokio::test]
async fn the_fallback_source_is_credited() {
    let rates = Server::start(vec![
        Reply::error(503, &[], "down"),
        json_reply(r#"{"result":"success","base_code":"USD","rates":{"USD":1,"CNY":7.0}}"#),
    ])
    .await;
    let home = Home::new();
    let (_core, mut client) = spent(&home, &rates).await;
    let reply = client.call("q1", "usage.query", json!({})).await;
    let converted = &reply["result"]["rows"][0]["converted"];
    assert!(
        (amount(&converted["amount"]) - (CNY + USD * 7.0)).abs() < 1e-12,
        "{reply}"
    );
    assert_eq!(
        reply["result"]["rates"],
        json!([{"source": "open.er-api.com", "url": "https://www.exchangerate-api.com"}]),
        "用到兜底源要注明来源"
    );
}

#[tokio::test]
async fn without_a_rate_the_other_currency_stays_apart() {
    // 汇率服务器一个都答不上：主源、兜底源都出错。
    let rates = Server::start(vec![
        Reply::error(500, &[], "boom"),
        Reply::error(500, &[], "boom"),
    ])
    .await;
    let home = Home::new();
    let (_core, mut client) = spent(&home, &rates).await;
    let reply = client.call("q1", "usage.query", json!({})).await;
    let converted = &reply["result"]["rows"][0]["converted"];
    assert!(
        (amount(&converted["amount"]) - CNY).abs() < 1e-12,
        "人民币本身那一笔不用汇率：{reply}"
    );
    assert_eq!(converted["rest"][0]["currency"], "USD", "{reply}");
    assert!((amount(&converted["rest"][0]["amount"]) - USD).abs() < 1e-12);
}

/// 会话状态里那一行照会话属主的币种换算（施工 UC-1）：常推的一处不等，只用缓存里的汇率；先查一回 `usage.query` 把汇率取下来，
/// 订阅回应里的累计就换成了人民币。
#[tokio::test]
async fn a_sessions_own_total_is_converted_for_its_owner() {
    use crate::support::providers::scripted;
    use miyu_models::catalog::{Price, Rates};
    use miyu_models::price::Tariff;
    use miyu_session::testkit::{Play, Script};

    let rates = Server::start(vec![json_reply(&format!(
        r#"[{{"date":"{}","base":"USD","quote":"CNY","rate":7.0}}]"#,
        today()
    ))])
    .await;
    let home = Home::new();
    home.write("config.toml", "[usage]\ncurrency = \"CNY\"\n");
    let usd = Tariff {
        price: Price::of(
            Rates {
                input: Some(1.0),
                output: Some(2.0),
                cache_read: Some(0.5),
                cache_write: None,
            },
            "USD",
        ),
        multiplier: 1.0,
        source: "local".to_string(),
    };
    let script = Script::new([Play::Says("好。")]).priced(usd);
    let core = scripted(&home, data(profiles(json!({}))), script);
    let base = rates.base_url.trim_end_matches("/v1");
    core.install_rates(Arc::new(HttpRates::at(
        client(Proxy::Off).expect("造得出"),
        Handle::current(),
        base,
        base,
    )));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    // 剧本一次报 60 没命中、40 命中、10 输出：美元 (60 + 40 × 0.5 + 10 × 2) / 1e6。
    let spent = (60.0 + 40.0 * 0.5 + 10.0 * 2.0) / 1e6;
    let reply = client
        .call("q1", "usage.query", json!({"session": session}))
        .await;
    assert!(
        (amount(&reply["result"]["rows"][0]["converted"]["amount"]) - spent * 7.0).abs() < 1e-12,
        "{reply}"
    );
    let subscribed = client.subscribe("w1", &session).await;
    let usage = &subscribed["result"]["usage"];
    assert_eq!(usage["converted"]["currency"], "CNY", "{subscribed}");
    assert!(
        (amount(&usage["converted"]["amount"]) - spent * 7.0).abs() < 1e-12,
        "{subscribed}"
    );
}
