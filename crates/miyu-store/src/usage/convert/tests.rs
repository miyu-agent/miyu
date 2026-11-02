//! 换算：同一天两笔不同币种换算加总；跨天各用各的汇率；美元自己是 1；要换成的币种本身不用汇率；缺一笔的留在 `rest`；
//! 一笔都没换成的没有；用到兜底源的标出来。

use std::collections::BTreeMap;

use miyu_kernel::event::Real;

use super::*;
use crate::usage::Parts;

fn table(rows: &[(Day, &str, f64, Source)]) -> BTreeMap<(Day, String), (f64, Source)> {
    rows.iter()
        .map(|(day, currency, rate, source)| ((*day, currency.to_string()), (*rate, *source)))
        .collect()
}

fn daily(rows: &[(Day, &str, f64)]) -> Vec<Daily> {
    rows.iter()
        .map(|(day, currency, amount)| Daily {
            day: *day,
            currency: currency.to_string(),
            amount: *amount,
            parts: None,
        })
        .collect()
}

/// 四种各多少，`None` 是那一种的价格不知道。
fn parts(four: [Option<f64>; 4]) -> Parts {
    let real = |part: Option<f64>| part.map(Real::new);
    Parts {
        uncached: real(four[0]),
        cache_read: real(four[1]),
        cache_write: real(four[2]),
        output: real(four[3]),
    }
}

/// 一笔：金额是四种加起来的（不知道的那种算 0）。
fn priced(day: Day, currency: &str, four: [Option<f64>; 4]) -> Daily {
    Daily {
        day,
        currency: currency.to_string(),
        amount: four.iter().flatten().sum(),
        parts: Some(parts(four)),
    }
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

/// 四种照同样的汇率换（施工 UP-1）：换完加起来等于 `amount`；没换成、留在 `rest` 的那一笔不进四种。
#[test]
fn the_four_kinds_convert_at_the_same_rate_and_add_up() {
    let rates = table(&[
        (10, "USD", 1.0, Source::Frankfurter),
        (10, "CNY", 7.0, Source::Frankfurter),
        (11, "USD", 1.0, Source::Frankfurter),
        (11, "CNY", 8.0, Source::Frankfurter),
    ]);
    let got = convert(
        &[
            priced(10, "USD", [Some(0.5), Some(0.25), Some(0.0), Some(1.0)]),
            priced(11, "USD", [Some(1.0), Some(0.0), Some(0.0), Some(2.0)]),
            priced(10, "CNY", [Some(3.0), Some(1.0), Some(0.5), Some(4.0)]),
            priced(10, "EUR", [Some(9.0), Some(9.0), Some(9.0), Some(9.0)]),
        ],
        "CNY",
        &rates,
    )
    .unwrap();
    let four = got.parts.expect("每一笔都有四种");
    let get = |part: Option<Real>| part.expect("价格都知道").get();
    assert!((get(four.uncached) - (0.5 * 7.0 + 8.0 + 3.0)).abs() < 1e-9);
    assert!((get(four.cache_read) - (0.25 * 7.0 + 1.0)).abs() < 1e-9);
    assert!((get(four.cache_write) - 0.5).abs() < 1e-9);
    assert!((get(four.output) - (7.0 + 16.0 + 4.0)).abs() < 1e-9);
    let sum: f64 = [
        four.uncached,
        four.cache_read,
        four.cache_write,
        four.output,
    ]
    .into_iter()
    .map(get)
    .sum();
    assert!(
        (sum - got.amount).abs() < 1e-9,
        "四种加起来就是总数：{got:?}"
    );
    assert_eq!(got.rest, vec![("EUR".to_string(), 36.0)]);
}

/// 有一笔没记四种的（老的账），整组不给四种；有一笔哪一种的价格不知道，那一种不给。
#[test]
fn a_charge_without_kinds_hides_them_all_and_an_unknown_price_hides_its_kind() {
    let rates = table(&[]);
    let mut old = priced(10, "CNY", [Some(1.0), Some(1.0), Some(1.0), Some(1.0)]);
    old.parts = None;
    let got = convert(
        &[
            priced(10, "CNY", [Some(1.0), Some(1.0), Some(1.0), Some(1.0)]),
            old,
        ],
        "CNY",
        &rates,
    )
    .unwrap();
    assert_eq!(got.parts, None, "不拼半截：{got:?}");
    let got = convert(
        &[
            priced(10, "CNY", [Some(1.0), Some(1.0), None, Some(1.0)]),
            priced(11, "CNY", [Some(1.0), Some(1.0), Some(1.0), Some(1.0)]),
        ],
        "CNY",
        &rates,
    )
    .unwrap();
    assert_eq!(
        got.parts,
        Some(parts([Some(2.0), Some(2.0), None, Some(2.0)])),
        "{got:?}"
    );
    // 没换成的那一笔价格不知道，也算这一组不知道。
    let got = convert(
        &[
            priced(10, "CNY", [Some(1.0), Some(1.0), Some(1.0), Some(1.0)]),
            priced(10, "EUR", [Some(1.0), None, Some(1.0), Some(1.0)]),
        ],
        "CNY",
        &rates,
    )
    .unwrap();
    assert_eq!(got.parts.and_then(|four| four.cache_read), None);
}
