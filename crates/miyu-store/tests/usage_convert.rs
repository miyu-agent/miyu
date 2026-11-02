//! 金额换算走真的用量汇总（施工 UC-1）：查询交每一组按 UTC 日子、币种分开的金额，照汇率换成一个币种；跨天各用各的汇率。
//! 另有一条标 `ignore` 的量尺：缓存命中时换算比不换算慢多少。

use std::sync::Arc;
use std::time::{Duration, Instant};

use miyu_kernel::event::Real;

use miyu_store::trash;
use miyu_store::usage::rates::{Day, Fetched, RateSource, Source, WAIT, day_of};
use miyu_store::usage::{Daily, Group, Parts, UsageIndex};

use crate::support::*;

/// 回收处留 7 天。
const WEEK: Duration = Duration::from_secs(7 * 24 * 3600);

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
    let plain = |daily: &[Daily]| -> Vec<(Day, String, f64)> {
        daily
            .iter()
            .map(|daily| (daily.day, daily.currency.clone(), daily.amount))
            .collect()
    };
    assert_eq!(
        plain(&total.daily),
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
    assert_eq!(
        plain(&days[1].daily),
        vec![(first + 1, "USD".to_string(), 1.0)]
    );
}

/// 一次请求的金额，四种价格里写缓存的不知道：每百万输入 1、读缓存 0.5、输出 2 美元，倍率 2。
const PRICED: &str = r#"{"amount":0.000036,"currency":"USD","price":{"input":1,"cache_read":0.5,"output":2},"multiplier":2,"source":"local"}"#;

/// 用量 10 没命中、4 读缓存、3 输出照 [`PRICED`] 的四种：写缓存的价格不知道，可用量是 0，就是准的 0。
fn four(times: f64) -> [Option<f64>; 4] {
    [
        Some(times * 10.0 * 2.0 / 1e6),
        Some(times * 4.0 * 0.5 * 2.0 / 1e6),
        Some(0.0),
        Some(times * 3.0 * 2.0 * 2.0 / 1e6),
    ]
}

/// 一组的四种，没有的是 `None`。
fn kinds(daily: &[Daily]) -> Option<[Option<f64>; 4]> {
    let mut sum = [Some(0.0); 4];
    for daily in daily {
        let parts: Parts = daily.parts?;
        let got = [
            parts.uncached,
            parts.cache_read,
            parts.cache_write,
            parts.output,
        ];
        for (sum, got) in sum.iter_mut().zip(got) {
            *sum = sum.zip(got).map(|(a, b)| a + b.get());
        }
    }
    Some(sum)
}

fn close(got: Option<[Option<f64>; 4]>, want: [Option<f64>; 4]) -> bool {
    got.is_some_and(|got| {
        got.iter().zip(want).all(|pair| match pair {
            (Some(a), Some(b)) => (a - b).abs() < 1e-15,
            (None, None) => true,
            _ => false,
        })
    })
}

/// 四种照每一次请求自己记下的价格算好、跟着那一次记进汇总（施工 UP-1）：重建照样有；删掉的会话的合计里也带着；老的合计
/// （没有四种的 `usage.purged`）算进来的那一组不给四种，别的组照给。
#[test]
fn each_charge_keeps_its_four_kinds_through_rebuilds_and_purges() {
    let scratch = Scratch::new("usage-parts");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    for n in [1, 2] {
        let id = session(n);
        let mut log = write_log(&root.session_dir(&id), &[]);
        let events = [
            created(at(0), None),
            called(2, at(2), "m", Some([10, 4, 0, 3]), PRICED, ""),
            called(3, at(4), "m", Some([10, 4, 0, 3]), PRICED, ""),
        ];
        land(&index, &mut log, &id, &who(None), &events);
    }
    let mut asked = query(&[]);
    asked.daily = true;
    let total = totals(&index, &asked).remove(0);
    assert!(close(kinds(&total.daily), four(4.0)), "{total:?}");
    let converted = index
        .convert(&total.daily, "USD", at(10), WAIT)
        .expect("美元本身不用汇率");
    let parts = converted.parts.expect("都有四种");
    let all = [
        parts.uncached,
        parts.cache_read,
        parts.cache_write,
        parts.output,
    ];
    assert!(all.iter().all(Option::is_some), "四种都在：{converted:?}");
    let sum: f64 = all.into_iter().flatten().map(Real::get).sum();
    assert!(
        (sum - converted.amount).abs() < 1e-15,
        "四种加起来就是总数：{converted:?}"
    );
    assert_eq!(
        parts.cache_write.map(Real::get),
        Some(0.0),
        "写缓存的价格不知道、用量是 0：准的 0"
    );
    // 删了汇总重建：照日志里记下的价格算出一样的。
    drop(index);
    forget(&root);
    let (index, _) = UsageIndex::open(&root);
    let rebuilt = totals(&index, &asked).remove(0);
    assert!(close(kinds(&rebuilt.daily), four(4.0)), "{rebuilt:?}");
    // 清掉会话 2：合计里带着四种。
    trash::discard(&root, &session(2), at(20)).expect("挪得进回收处");
    let purged = trash::purge(&root, at(20 + 8 * 24 * 60), WEEK).expect("回收处读得了");
    assert_eq!(purged.removed, 1);
    let after = totals(&index, &asked).remove(0);
    assert!(close(kinds(&after.daily), four(4.0)), "{after:?}");
    // 一条老的合计（没有四种）：算进它的那一组不给，别的组照给。
    let old = format!(
        r#"{{"session":"{}","venue":"local","spent":[{{"provider":"deepseek","model":"m","hour":"2026-10-01T00:00:00.000Z","requests":1,"unpriced":0,"usage":{{"uncached":1,"cache_read":0,"cache_write":0,"output":0}},"amounts":[{{"currency":"USD","amount":0.5}}]}}]}}"#,
        session(3).as_str()
    );
    miyu_store::journal::append(
        &root.journal_file(),
        miyu_store::journal::Entry {
            at: at(30),
            by: miyu_kernel::origin::By::Kernel,
            cause: None,
            kind: miyu_kernel::id::EventKind::parse("usage.purged").expect("合写法"),
            body: serde_json::from_str(&old).expect("合写法"),
        },
    )
    .expect("写得进");
    let mixed = totals(&index, &asked).remove(0);
    assert_eq!(kinds(&mixed.daily), None, "不拼半截：{mixed:?}");
    let converted = index
        .convert(&mixed.daily, "USD", at(40), WAIT)
        .expect("美元本身不用汇率");
    assert_eq!(converted.parts, None);
    let mut by_session = query(&[Group::Session]);
    by_session.daily = true;
    let rows = totals(&index, &by_session);
    assert_eq!(rows.len(), 3);
    assert!(close(kinds(&rows[0].daily), four(2.0)), "会话 1：{rows:?}");
    assert!(
        close(kinds(&rows[1].daily), four(2.0)),
        "会话 2 删了也有：{rows:?}"
    );
    assert_eq!(kinds(&rows[2].daily), None, "老的合计：{rows:?}");
}

/// 只写了输入、输出价的模型（施工 UP-1，核心主会话 2026-11-02 定）：缓存两种用量是 0 的，照样是准的 0；有一笔读缓存用量
/// 不是 0、价格不知道的，这一组的读缓存不给，别的照给。
#[test]
fn a_zero_usage_is_an_exact_zero_and_an_unpriced_usage_is_left_out() {
    let scratch = Scratch::new("usage-parts-zero");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    let id = session(1);
    let mut log = write_log(&root.session_dir(&id), &[]);
    // 价格只有输入 1、输出 2（`support::cost`），倍率 1。
    let events = [
        created(at(0), None),
        called(
            2,
            at(2),
            "m",
            Some([10, 0, 0, 1]),
            &cost("0.000012", "USD"),
            "",
        ),
    ];
    land(&index, &mut log, &id, &who(None), &events);
    let mut asked = query(&[]);
    asked.daily = true;
    let total = totals(&index, &asked).remove(0);
    assert!(
        close(
            kinds(&total.daily),
            [Some(10.0 / 1e6), Some(0.0), Some(0.0), Some(2.0 / 1e6)]
        ),
        "缓存两种用量是 0，照 0 给：{total:?}"
    );
    // 又一笔读了 5 个缓存，读缓存的价格不知道。
    land(
        &index,
        &mut log,
        &id,
        &who(None),
        &[called(
            3,
            at(4),
            "m",
            Some([20, 5, 0, 2]),
            &cost("0.000024", "USD"),
            "",
        )],
    );
    let total = totals(&index, &asked).remove(0);
    assert!(
        close(
            kinds(&total.daily),
            [Some(30.0 / 1e6), None, Some(0.0), Some(6.0 / 1e6)]
        ),
        "读缓存不给，别的照给：{total:?}"
    );
    let parts = index
        .convert(&total.daily, "USD", at(10), WAIT)
        .and_then(|converted| converted.parts)
        .expect("有四种");
    assert_eq!(parts.cache_read, None);
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
