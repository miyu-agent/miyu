//! 换算：同一天两笔不同币种换算加总；跨天各用各的汇率；美元自己是 1；要换成的币种本身不用汇率；缺一笔的留在 `rest`；
//! 一笔都没换成的没有；用到兜底源的标出来。

use std::collections::BTreeMap;

use super::*;

fn table(rows: &[(Day, &str, f64, Source)]) -> BTreeMap<(Day, String), (f64, Source)> {
    rows.iter()
        .map(|(day, currency, rate, source)| ((*day, currency.to_string()), (*rate, *source)))
        .collect()
}

fn daily(rows: &[(Day, &str, f64)]) -> Vec<(Day, String, f64)> {
    rows.iter()
        .map(|(day, currency, amount)| (*day, currency.to_string(), *amount))
        .collect()
}

#[test]
fn two_currencies_on_one_day_add_up_in_the_target() {
    let rates = table(&[
        (10, "USD", 1.0, Source::Frankfurter),
        (10, "CNY", 7.0, Source::Frankfurter),
    ]);
    let got = convert(
        &daily(&[(10, "USD", 1.0), (10, "CNY", 14.0)]),
        "CNY",
        &rates,
    )
    .unwrap();
    assert_eq!(got.currency, "CNY");
    assert!((got.amount - 21.0).abs() < 1e-9, "{got:?}");
    assert!(got.rest.is_empty());
    assert!(!got.fallback);
}

#[test]
fn each_day_uses_its_own_rate() {
    let rates = table(&[
        (10, "USD", 1.0, Source::Frankfurter),
        (10, "CNY", 7.0, Source::Frankfurter),
        (11, "USD", 1.0, Source::Frankfurter),
        (11, "CNY", 8.0, Source::Frankfurter),
    ]);
    let got = convert(&daily(&[(10, "USD", 1.0), (11, "USD", 1.0)]), "CNY", &rates).unwrap();
    assert!((got.amount - 15.0).abs() < 1e-9, "{got:?}");
}

#[test]
fn the_target_needs_no_rate_and_a_missing_rate_stays_in_the_rest() {
    let rates = table(&[(10, "CNY", 7.0, Source::Fallback)]);
    let got = convert(
        &daily(&[(10, "CNY", 3.0), (10, "EUR", 2.0), (11, "USD", 1.0)]),
        "CNY",
        &rates,
    )
    .unwrap();
    assert!(
        (got.amount - 3.0).abs() < 1e-9,
        "人民币本身不用汇率：{got:?}"
    );
    assert_eq!(
        got.rest,
        vec![("EUR".to_string(), 2.0), ("USD".to_string(), 1.0)]
    );
    assert!(!got.fallback, "没用到汇率，不算用了兜底源");
    let used = convert(
        &daily(&[(10, "USD", 1.0)]),
        "CNY",
        &table(&[
            (10, "USD", 1.0, Source::Frankfurter),
            (10, "CNY", 7.0, Source::Fallback),
        ]),
    )
    .unwrap();
    assert!(used.fallback, "用到了兜底源");
}

#[test]
fn nothing_converted_is_nothing() {
    assert_eq!(
        convert(&daily(&[(10, "EUR", 2.0)]), "CNY", &BTreeMap::new()),
        None
    );
    assert_eq!(convert(&[], "CNY", &BTreeMap::new()), None);
}
