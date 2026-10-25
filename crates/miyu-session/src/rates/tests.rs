//! 取汇率，对着假服务器：一个币种缺的几天并成一段一次取，请求里只有币种和日子；正好那一天、那一天过完了的定了，当天的、
//! 交回的日子比要的早的暂定；源说不认识的币种记成没有；主源出错的，当天、昨天照兜底源，更早的不给。

use miyu_http::testkit::{Reply, Server};
use miyu_http::{Proxy, client};

use super::*;

/// 2026-10-20 是第几天。
fn d(date: &str) -> Day {
    day(date).expect("合写法")
}

fn json(body: &str) -> Reply {
    Reply::error(200, &[("Content-Type", "application/json")], body)
}

async fn rates(server: &Server) -> HttpRates {
    let base = server.base_url.trim_end_matches("/v1");
    HttpRates::at(
        client(Proxy::Off).expect("造得出"),
        Handle::current(),
        base,
        base,
    )
}

#[tokio::test]
async fn the_missing_days_of_a_currency_go_in_one_request_with_only_dates_and_codes() {
    let server = Server::start(vec![json(
        r#"[{"date":"2026-10-18","base":"USD","quote":"CNY","rate":7.1},
            {"date":"2026-10-19","base":"USD","quote":"CNY","rate":7.2},
            {"date":"2026-10-20","base":"USD","quote":"CNY","rate":7.3}]"#,
    )])
    .await;
    let source = rates(&server).await;
    let got = source
        .fetch(
            vec![
                (d("2026-10-18"), "CNY".to_string()),
                (d("2026-10-20"), "CNY".to_string()),
            ],
            d("2026-10-24"),
        )
        .await;
    assert_eq!(
        got,
        vec![
            Fetched {
                day: d("2026-10-18"),
                currency: "CNY".to_string(),
                rate: Some(7.1),
                source: Source::Frankfurter,
                settled: true,
            },
            Fetched {
                day: d("2026-10-20"),
                currency: "CNY".to_string(),
                rate: Some(7.3),
                source: Source::Frankfurter,
                settled: true,
            },
        ]
    );
    let received = server.received();
    assert_eq!(received.len(), 1, "并成一次");
    assert_eq!(
        received[0].path, "/v2/rates?base=USD&quotes=CNY&from=2026-10-18&to=2026-10-20",
        "只有币种和日子"
    );
}

#[tokio::test]
async fn today_and_an_earlier_answer_are_provisional() {
    let server = Server::start(vec![json(
        r#"[{"date":"2026-10-23","base":"USD","quote":"EUR","rate":0.9}]"#,
    )])
    .await;
    let got = rates(&server)
        .await
        .fetch(vec![(d("2026-10-24"), "EUR".to_string())], d("2026-10-24"))
        .await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].rate, Some(0.9), "用更早最近的一天");
    assert!(!got[0].settled, "当天的、交回的日子比要的早，暂定");
    // 那一天已经过完了，交回的还是更早一天的：照样暂定。
    let server = Server::start(vec![json(
        r#"[{"date":"2026-10-21","base":"USD","quote":"EUR","rate":0.8}]"#,
    )])
    .await;
    let got = rates(&server)
        .await
        .fetch(vec![(d("2026-10-22"), "EUR".to_string())], d("2026-10-24"))
        .await;
    assert_eq!(got[0].rate, Some(0.8));
    assert!(!got[0].settled, "不是那一天的，不算定数");
}

#[tokio::test]
async fn an_unknown_currency_has_no_rate() {
    let server = Server::start(vec![Reply::error(
        422,
        &[("Content-Type", "application/json")],
        r#"{"status":422,"message":"invalid currency: XYZ"}"#,
    )])
    .await;
    let got = rates(&server)
        .await
        .fetch(vec![(d("2026-10-20"), "XYZ".to_string())], d("2026-10-24"))
        .await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].rate, None);
    assert!(!got[0].settled, "过期了再问");
}

#[tokio::test]
async fn when_the_main_source_fails_today_and_yesterday_use_the_fallback() {
    let server = Server::start(vec![
        Reply::error(500, &[], "boom"),
        json(r#"{"result":"success","base_code":"USD","rates":{"USD":1,"CNY":7.0}}"#),
    ])
    .await;
    let got = rates(&server)
        .await
        .fetch(
            vec![
                (d("2026-10-20"), "CNY".to_string()),
                (d("2026-10-23"), "CNY".to_string()),
                (d("2026-10-24"), "CNY".to_string()),
            ],
            d("2026-10-24"),
        )
        .await;
    let days: Vec<(Day, Option<f64>, Source, bool)> = got
        .iter()
        .map(|row| (row.day, row.rate, row.source, row.settled))
        .collect();
    assert_eq!(
        days,
        vec![
            (d("2026-10-23"), Some(7.0), Source::Fallback, false),
            (d("2026-10-24"), Some(7.0), Source::Fallback, false),
        ],
        "更早的日子不拿最新的去换"
    );
    assert_eq!(server.received()[1].path, "/v6/latest/USD");
}
