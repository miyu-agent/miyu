//! `usage.convert`（施工 UC-3，`docs/blueprint/models.md`「协议」`usage.convert`）：扩展自己照日子、币种记的金额交给核心换算。
//! 一组换成一个数，照组的先后交回；同币种的不用汇率；空的组、一笔都没换成的是 `null`；取不到汇率的那一笔在 `rest`；
//! 回应的 `currency` 跟着配置；参数写错、超上限的 `bad_params`。汇率从假的汇率服务器取。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::runtime::Handle;

use crate::support::providers::{data, profiles, routed};
use crate::support::*;
use miyu_endpoint::Core;
use miyu_http::testkit::{Reply, Server};
use miyu_http::{Proxy, client};
use miyu_session::rates::HttpRates;

/// 早就过完的两天：汇率是定了的，只问主源。
const ONE: &str = "2025-03-03";
const TWO: &str = "2025-03-04";

fn json_reply(body: &str) -> Reply {
    Reply::error(200, &[("Content-Type", "application/json")], body)
}

/// 起核心（`usage` 是配置里那一段），装上对着 `rates` 的取汇率。
fn core(home: &Home, usage: &str, rates: &Server) -> Arc<Core> {
    home.write("etc/config.toml", usage);
    let core = routed(home, &[], data(profiles(json!({}))));
    let base = rates.base_url.trim_end_matches("/v1");
    core.install_rates(Arc::new(HttpRates::at(
        client(Proxy::Off).expect("造得出"),
        Handle::current(),
        base,
        base,
    )));
    core
}

fn amount(value: &Value) -> f64 {
    value.as_f64().unwrap_or_else(|| panic!("是数：{value}"))
}

#[tokio::test]
async fn each_group_becomes_one_amount_at_the_days_rates() {
    // 汇率照币种一个一个问，先人民币（两天）、再欧元（第二天）。
    let rates = Server::start(vec![
        json_reply(&format!(
            r#"[{{"date":"{ONE}","base":"USD","quote":"CNY","rate":7.0}},{{"date":"{TWO}","base":"USD","quote":"CNY","rate":7.2}}]"#
        )),
        json_reply(&format!(
            r#"[{{"date":"{TWO}","base":"USD","quote":"EUR","rate":0.9}}]"#
        )),
    ])
    .await;
    let home = Home::new();
    let core = core(&home, "[usage]\ncurrency = \"CNY\"\n", &rates);
    let mut client = Client::connect(core);
    client.hello().await;
    let groups = json!({"groups": [
        [{"day": ONE, "currency": "USD", "amount": 1.0}, {"day": TWO, "currency": "EUR", "amount": 2.0}],
        [{"day": ONE, "currency": "CNY", "amount": 3.5}, {"day": TWO, "currency": "CNY", "amount": 1.5}],
        [],
    ]});
    let reply = client.call("v1", "usage.convert", groups.clone()).await;
    let result = &reply["result"];
    assert_eq!(result["currency"], "CNY", "{reply}");
    let converted = result["converted"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(converted.len(), 3, "一组一项：{reply}");
    assert_eq!(converted[0]["currency"], "CNY", "{reply}");
    assert!(
        (amount(&converted[0]["amount"]) - (1.0 * 7.0 + 2.0 / 0.9 * 7.2)).abs() < 1e-9,
        "美元那一笔照第一天乘 7，欧元那一笔照第二天经美元换：{reply}"
    );
    assert!(converted[0].get("rest").is_none(), "都换成了：{reply}");
    assert_eq!(
        converted[1],
        json!({"currency": "CNY", "amount": 5}),
        "同币种的原样加起来：{reply}"
    );
    assert_eq!(converted[2], Value::Null, "空的组是 null：{reply}");
    // 再问不再取：几组的汇率只找一次，取到的记下了。
    let again = client.call("v2", "usage.convert", groups).await;
    assert_eq!(again["result"], *result);
    assert_eq!(rates.received().len(), 2, "一个币种只问一次");
}

#[tokio::test]
async fn an_amount_without_a_rate_stays_in_rest() {
    let rates = Server::start(vec![
        Reply::error(500, &[], "boom"),
        Reply::error(500, &[], "boom"),
    ])
    .await;
    let home = Home::new();
    let core = core(&home, "[usage]\ncurrency = \"CNY\"\n", &rates);
    let mut client = Client::connect(core);
    client.hello().await;
    let groups = json!({"groups": [
        [{"day": ONE, "currency": "CNY", "amount": 1.25}, {"day": ONE, "currency": "USD", "amount": 2.0}],
        [{"day": ONE, "currency": "USD", "amount": 2.0}],
    ]});
    let reply = client.call("v1", "usage.convert", groups).await;
    let converted = &reply["result"]["converted"];
    assert_eq!(
        converted[0],
        json!({"currency": "CNY", "amount": 1.25, "rest": [{"currency": "USD", "amount": 2}]}),
        "人民币本身那一笔不用汇率，美元那一笔留在 rest：{reply}"
    );
    assert_eq!(converted[1], Value::Null, "一笔都没换成的是 null：{reply}");
}

#[tokio::test]
async fn the_currency_follows_the_setting() {
    let rates = Server::start(vec![]).await;
    let home = Home::new();
    // 没写 `usage.currency`：默认美元。美元换美元不用汇率。
    let core = core(&home, "", &rates);
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client
        .call(
            "v1",
            "usage.convert",
            json!({"groups": [[{"day": ONE, "currency": "USD", "amount": 0.12}]]}),
        )
        .await;
    assert_eq!(
        reply["result"],
        json!({"currency": "USD", "converted": [{"currency": "USD", "amount": 0.12}]}),
        "{reply}"
    );
    let none = client
        .call("v2", "usage.convert", json!({"groups": []}))
        .await;
    assert_eq!(
        none["result"],
        json!({"currency": "USD", "converted": []}),
        "一组都没有：{none}"
    );
    assert!(rates.received().is_empty(), "用不着汇率的不去取");
}

#[tokio::test]
async fn wrong_params_and_too_many_are_bad_params() {
    let rates = Server::start(vec![]).await;
    let home = Home::new();
    let core = core(&home, "", &rates);
    let mut client = Client::connect(core);
    client.hello().await;
    let entry = json!({"day": ONE, "currency": "USD", "amount": 1});
    let one = |entry: Value| json!({"groups": [[entry]]});
    for (n, params) in [
        json!({}),
        json!({"groups": "all"}),
        json!({"groups": [entry.clone()]}),
        one(json!({"day": "2025-13-01", "currency": "USD", "amount": 1})),
        one(json!({"day": "2025-02-30", "currency": "USD", "amount": 1})),
        one(json!({"day": "20250303", "currency": "USD", "amount": 1})),
        one(json!({"day": "2025-03-03T00:00:00.000Z", "currency": "USD", "amount": 1})),
        one(json!({"day": 20250303, "currency": "USD", "amount": 1})),
        one(json!({"day": ONE, "currency": "USD", "amount": "0.12"})),
        one(json!({"day": ONE, "currency": "USD", "amount": null})),
        one(json!({"day": ONE, "currency": "USD"})),
        one(json!({"day": ONE, "currency": "US", "amount": 1})),
        one(json!({"day": ONE, "currency": "CNY&quotes=EUR", "amount": 1})),
        one(json!({"day": ONE, "amount": 1})),
        // 超上限：1001 组、一共 20001 笔。
        json!({"groups": vec![json!([]); 1001]}),
        json!({"groups": [vec![entry.clone(); 10_000], vec![entry.clone(); 10_001]]}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("b{n}"), "usage.convert", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "第 {n} 个：{reply}");
    }
    // 正好在上限上的照办：1000 组，一共 20000 笔。
    let mut groups = vec![json!([]); 998];
    groups.push(json!(vec![entry.clone(); 10_000]));
    groups.push(json!(vec![entry; 10_000]));
    let reply = client
        .call("ok", "usage.convert", json!({"groups": groups}))
        .await;
    let converted = &reply["result"]["converted"];
    assert_eq!(converted.as_array().map(Vec::len), Some(1000), "照办");
    assert_eq!(converted[998], json!({"currency": "USD", "amount": 10_000}));
    assert!(rates.received().is_empty());
}
