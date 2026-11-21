//! 在随机的时刻收成槽再放回（施工 LW-10 下，`watch/shelf.rs`）：和同一个种子一直不收的那一趟比。收不收用的是另一串随机数，两趟
//! 送进去的输入一条不差。

use super::watch::shelf;
use super::*;

/// 在随机的时刻收成槽再放回（施工 LW-10 下，`watch/shelf.rs`），和同一个种子一直不收的那一趟比：每一次请求（主请求、辅助请求、
/// 看图）照内核的写法逐字节一样，日志一字不差。种子 `seeds`，交回收了几次。
fn same_without_shelving(seeds: std::ops::Range<u64>) -> usize {
    let mut shelvings = 0;
    for seed in seeds {
        let (shelved, kept) = (drive(seed), shelf::never(|| drive(seed)));
        assert_eq!(kept.shelf.shelvings, 0, "种子 {seed}：不收的那一趟收过");
        assert!(
            shelved.shelf.asked == kept.shelf.asked,
            "种子 {seed}：收过再放回的请求和一直不收的不一样"
        );
        let lines =
            |watch: &Watch| -> Vec<String> { watch.log().iter().map(Event::to_line).collect() };
        assert_eq!(lines(&shelved), lines(&kept), "种子 {seed}：日志不一样");
        shelvings += shelved.shelf.shelvings;
    }
    shelvings
}

/// 平时跑的六十例：收过再放回的请求、日志和一直不收的一样，真收过。
#[test]
fn shelving_changes_nothing() {
    let shelvings = same_without_shelving(0..60);
    assert!(shelvings >= 200, "六十例只收了 {shelvings} 次");
}

/// 全量：接着往后跑两千例（CI 的长跑那一项用 `--ignored`、release 模式跑）。
#[test]
#[ignore = "长跑，CI 的长跑那一项用 --ignored 跑（施工 LW-10 下）"]
fn shelving_changes_nothing_for_longer() {
    let shelvings = same_without_shelving(60..2060);
    assert!(shelvings >= 6_000, "两千例只收了 {shelvings} 次");
}
