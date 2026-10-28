//! 金额换算走真的用量汇总（施工 UC-1）：查询交每一组按 UTC 日子、币种分开的金额，照汇率换成一个币种；跨天各用各的汇率。
//! 另有一条标 `ignore` 的量尺：缓存命中时换算比不换算慢多少。

use std::sync::Arc;
use std::time::{Duration, Instant};

use miyu_store::usage::rates::{Day, Fetched, RateSource, Source, WAIT, day_of};
use miyu_store::usage::{Group, UsageIndex};

use crate::support::*;

/// 1 美元：10 月 1 日换 7 人民币，别的日子换 8。
struct Rates;

impl RateSource for Rates {
    fn start(
        &self,
        wanted: Vec<(Day, String)>,
        _today: Day,
        done: Box<dyn FnOnce(Vec<Fetched>) + Send>,
    ) {
        let first = day_of(at(0).unix_millis());
        done(
            wanted
                .into_iter()
                .map(|(day, currency)| Fetched {
                    day,
                    rate: (currency == "CNY").then_some(if day == first { 7.0 } else { 8.0 }),
                    currency,
                    source: Source::Frankfurter,
                    settled: true,
                })
                .collect(),
        );
    }
}

/// 一个主会话：10 月 1 日美元 0.25、人民币 1.5，10 月 2 日美元 1。
fn spent(root: &miyu_store::root::DataRoot, index: &UsageIndex) {
    let id = session(1);
    let mut log = write_log(&root.session_dir(&id), &[]);
    let events = [
        created(at(0), None),
        called(2, at(2), "m", Some([1, 0, 0, 1]), &cost("0.25", "USD"), ""),
        called(3, at(4), "m", Some([1, 0, 0, 1]), &cost("1.5", "CNY"), ""),
        called(
            4,
            at(24 * 60 + 5),
            "m",
            Some([1, 0, 0, 1]),
            &cost("1", "USD"),
            "",
        ),
    ];
    land(index, &mut log, &id, &who(None), &events);
}

#[test]
fn a_query_hands_amounts_by_day_and_they_convert_at_each_days_rate() {
    let scratch = Scratch::new("usage-convert");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    index.rates().install(Arc::new(Rates));
    spent(&root, &index);
    let mut asked = query(&[]);
    assert!(totals(&index, &asked)[0].daily.is_empty(), "没要的不交");
    asked.daily = true;
    let total = totals(&index, &asked).remove(0);
    let first = day_of(at(0).unix_millis());
    assert_eq!(
        total.daily,
        vec![
            (first, "CNY".to_string(), 1.5),
            (first, "USD".to_string(), 0.25),
            (first + 1, "USD".to_string(), 1.0),
        ]
    );
    let converted = index
        .convert(&total.daily, "CNY", at(3 * 24 * 60), WAIT)
        .expect("换成了");
    assert!(
        (converted.amount - (1.5 + 0.25 * 7.0 + 8.0)).abs() < 1e-9,
        "跨天各用各的汇率：{converted:?}"
    );
    // 分组的照组各算各的。
    let mut by_day = query(&[Group::Day]);
    by_day.daily = true;
    let days = totals(&index, &by_day);
    assert_eq!(days.len(), 2);
    assert_eq!(days[1].daily, vec![(first + 1, "USD".to_string(), 1.0)]);
}

/// 量尺（`cargo test -p miyu-store --test all usage_convert::bench -- --ignored --nocapture`）：五千行、三十天、两种币种，
/// 汇率都在缓存里时，查询加换算和只查询各自的 p95。
#[test]
#[ignore = "量尺"]
fn bench_conversion_with_cached_rates() {
    let scratch = Scratch::new("usage-convert-bench");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    index.rates().install(Arc::new(Rates));
    let id = session(1);
    let mut log = write_log(&root.session_dir(&id), &[]);
    let mut events = vec![created(at(0), None)];
    for n in 0..5000u64 {
        let minutes = i64::try_from(n).expect("放得下") * 8;
        let currency = if n % 2 == 0 { "USD" } else { "CNY" };
        events.push(called(
            n + 2,
            at(minutes),
            "m",
            Some([1, 0, 0, 1]),
            &cost("0.01", currency),
            "",
        ));
    }
    land(&index, &mut log, &id, &who(None), &events);
    let plain = query(&[]);
    let mut daily = query(&[]);
    daily.daily = true;
    let now = at(40 * 24 * 60);
    // 先取一遍汇率进缓存。
    let total = totals(&index, &daily).remove(0);
    index.convert(&total.daily, "CNY", now, WAIT);
    let p95 = |run: &dyn Fn()| {
        let mut took: Vec<Duration> = (0..50)
            .map(|_| {
                let started = Instant::now();
                run();
                started.elapsed()
            })
            .collect();
        took.sort();
        took[47]
    };
    let before = p95(&|| {
        index.query(&plain).expect("查得了");
    });
    let after = p95(&|| {
        let total = index.query(&daily).expect("查得了").remove(0);
        index.convert(&total.daily, "CNY", now, Duration::ZERO);
    });
    println!("只查询 p95 {before:?}，查询加换算 p95 {after:?}");
}
