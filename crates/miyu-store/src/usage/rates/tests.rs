//! 找汇率：缺的去取、取到了记下，再找不再取；同时缺的并成一次、同一个缺口跟着等；等不到的这次没有、回来以后下次有；
//! 暂定过期的照用旧的、在后面重取；源不认识的币种记下、新鲜时不再问；美元是 1；没装端口的只用缓存；坏了的库重建。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use super::*;

/// 这一刻：第 100 天的中午。
const NOW: i64 = 100 * DAY_MS + 12 * 3_600_000;

/// 假的取汇率端口：照 `rates` 答（没有的币种答空），过 `delay` 再交；记下每一次要的。
struct Fake {
    rates: BTreeMap<String, f64>,
    delay: Duration,
    asked: Mutex<Vec<Vec<(Day, String)>>>,
}

impl Fake {
    fn new(rates: &[(&str, f64)], delay: Duration) -> Arc<Fake> {
        Arc::new(Fake {
            rates: rates
                .iter()
                .map(|(currency, rate)| (currency.to_string(), *rate))
                .collect(),
            delay,
            asked: Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> Vec<Vec<(Day, String)>> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl RateSource for Fake {
    fn start(
        &self,
        wanted: Vec<(Day, String)>,
        today: Day,
        done: Box<dyn FnOnce(Vec<Fetched>) + Send>,
    ) {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(wanted.clone());
        let fetched: Vec<Fetched> = wanted
            .into_iter()
            .map(|(day, currency)| Fetched {
                day,
                rate: self.rates.get(&currency).copied(),
                currency,
                source: Source::Frankfurter,
                settled: day < today,
            })
            .collect();
        let delay = self.delay;
        thread::spawn(move || {
            thread::sleep(delay);
            done(fetched);
        });
    }
}

fn book(dir: &std::path::Path, source: Option<Arc<Fake>>) -> Arc<RateBook> {
    let book = Arc::new(RateBook::open(&dir.join(FILE)));
    if let Some(source) = source {
        book.install(source);
    }
    book
}

fn wanted(pairs: &[(Day, &str)]) -> BTreeSet<(Day, String)> {
    pairs
        .iter()
        .map(|(day, currency)| (*day, currency.to_string()))
        .collect()
}

#[test]
fn a_missing_rate_is_fetched_once_and_then_read_from_the_cache() {
    let dir = crate::test_support::Scratch::new();
    let source = Fake::new(&[("CNY", 7.0), ("EUR", 0.9)], Duration::ZERO);
    let book = book(dir.rooted(), Some(Arc::clone(&source)));
    let asked = wanted(&[(99, "CNY"), (99, "EUR"), (99, "USD")]);
    let got = book.rates(&asked, NOW, WAIT);
    assert_eq!(
        got.get(&(99, "CNY".to_string())),
        Some(&(7.0, Source::Frankfurter))
    );
    assert_eq!(
        got.get(&(99, "USD".to_string())),
        Some(&(1.0, Source::Frankfurter)),
        "美元是 1"
    );
    assert_eq!(
        source.asked(),
        vec![vec![(99, "CNY".to_string()), (99, "EUR".to_string())]],
        "缺的并成一次，不问美元"
    );
    let again = book.rates(&asked, NOW, WAIT);
    assert_eq!(again, got);
    assert_eq!(source.asked().len(), 1, "缓存命中不再取");
    // 重开一份（重启核心）也读得到。
    let reopened = self::book(dir.rooted(), Some(Arc::clone(&source)));
    assert_eq!(reopened.rates(&asked, NOW, WAIT), got);
    assert_eq!(source.asked().len(), 1);
}

#[test]
fn the_same_gap_asked_at_once_is_fetched_once() {
    let dir = crate::test_support::Scratch::new();
    let source = Fake::new(&[("CNY", 7.0)], Duration::from_millis(200));
    let book = book(dir.rooted(), Some(Arc::clone(&source)));
    let asked = wanted(&[(99, "CNY")]);
    let threads: Vec<_> = (0..3)
        .map(|_| {
            let (book, asked) = (Arc::clone(&book), asked.clone());
            thread::spawn(move || book.rates(&asked, NOW, WAIT))
        })
        .collect();
    for thread in threads {
        let got = thread.join().unwrap();
        assert_eq!(
            got.get(&(99, "CNY".to_string())),
            Some(&(7.0, Source::Frankfurter))
        );
    }
    assert_eq!(source.asked().len(), 1, "跟着等，不再取一次");
}

#[test]
fn a_late_rate_is_missed_this_time_and_kept_for_next_time() {
    let dir = crate::test_support::Scratch::new();
    let source = Fake::new(&[("CNY", 7.0)], Duration::from_millis(300));
    let book = book(dir.rooted(), Some(Arc::clone(&source)));
    let asked = wanted(&[(99, "CNY")]);
    assert!(
        book.rates(&asked, NOW, Duration::from_millis(20))
            .is_empty(),
        "等不到的这次没有"
    );
    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        book.rates(&asked, NOW, Duration::from_millis(20))
            .get(&(99, "CNY".to_string())),
        Some(&(7.0, Source::Frankfurter)),
        "回来以后记下了"
    );
    assert_eq!(source.asked().len(), 1);
}

#[test]
fn a_stale_provisional_rate_is_used_and_fetched_again_behind() {
    let dir = crate::test_support::Scratch::new();
    let source = Fake::new(&[("CNY", 7.5)], Duration::from_millis(100));
    let book = book(dir.rooted(), Some(Arc::clone(&source)));
    // 第 100 天早上取的当天的：暂定。
    let morning = 100 * DAY_MS + 3_600_000;
    book.cache.put(
        &[Fetched {
            day: 100,
            currency: "CNY".to_string(),
            rate: Some(7.0),
            source: Source::Fallback,
            settled: false,
        }],
        morning,
    );
    let asked = wanted(&[(100, "CNY")]);
    assert_eq!(
        book.rates(&asked, morning + 3_600_000, Duration::ZERO)
            .get(&(100, "CNY".to_string())),
        Some(&(7.0, Source::Fallback))
    );
    assert!(source.asked().is_empty(), "一小时以内还新鲜，不重取");
    let got = book.rates(&asked, NOW, Duration::ZERO);
    assert_eq!(
        got.get(&(100, "CNY".to_string())),
        Some(&(7.0, Source::Fallback)),
        "过了 6 小时：照用旧的，不等"
    );
    assert_eq!(source.asked().len(), 1, "在后面重取");
    thread::sleep(Duration::from_millis(300));
    assert_eq!(
        book.rates(&asked, NOW, Duration::ZERO)
            .get(&(100, "CNY".to_string())),
        Some(&(7.5, Source::Frankfurter))
    );
    // 过了那一天：重取。
    book.rates(&asked, 101 * DAY_MS + 60_000, Duration::ZERO);
    assert_eq!(source.asked().len(), 2, "那天过完了重取");
}

#[test]
fn an_unknown_currency_is_remembered_and_not_asked_again_while_fresh() {
    let dir = crate::test_support::Scratch::new();
    let source = Fake::new(&[], Duration::ZERO);
    let book = book(dir.rooted(), Some(Arc::clone(&source)));
    let asked = wanted(&[(100, "XYZ")]);
    assert!(book.rates(&asked, NOW, WAIT).is_empty());
    assert!(book.rates(&asked, NOW + 60_000, WAIT).is_empty());
    assert_eq!(source.asked().len(), 1);
}

#[test]
fn without_a_source_only_the_cache_is_read() {
    let dir = crate::test_support::Scratch::new();
    let book = book(dir.rooted(), None);
    assert!(book.rates(&wanted(&[(99, "CNY")]), NOW, WAIT).is_empty());
}

#[test]
fn a_broken_cache_is_rebuilt() {
    let dir = crate::test_support::Scratch::new();
    std::fs::create_dir_all(dir.rooted()).unwrap();
    std::fs::write(dir.rooted().join(FILE), b"not a database").unwrap();
    let source = Fake::new(&[("CNY", 7.0)], Duration::ZERO);
    let book = book(dir.rooted(), Some(source));
    assert_eq!(
        book.rates(&wanted(&[(99, "CNY")]), NOW, WAIT).len(),
        1,
        "删掉重建，照样取、照样记"
    );
}
