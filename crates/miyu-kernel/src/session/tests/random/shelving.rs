//! 在随机的时刻收成槽再放回（施工 LW-10 下，`watch/shelf.rs`）：和同一个种子一直不收的那一趟比。收不收用的是另一串随机数，两趟
//! 送进去的输入一条不差。收着时来的旁听的话、`events.append`、空闲时的「落了盘」不放回、直接送（施工 LW-36），照样一样。

use super::watch::shelf;
use super::*;

/// 在随机的时刻收成槽再放回（施工 LW-10 下，`watch/shelf.rs`），和同一个种子一直不收的那一趟比：每一次请求（主请求、辅助请求、
/// 看图）照内核的写法逐字节一样，日志一字不差。种子 `seeds`，交回收了几次、收着时不放回直接送了几条。
///
/// `overhear` 的：闲着时插旁听的话、`events.append`（施工 LW-36，`watch/shelf.rs` 的 `overhear`），两趟插得一样。
fn same_without_shelving(seeds: std::ops::Range<u64>, overhear: bool) -> (usize, usize) {
    let (mut shelvings, mut quiet) = (0, 0);
    for seed in seeds {
        let both = || (drive(seed), shelf::never(|| drive(seed)));
        let (shelved, kept) = match overhear {
            true => shelf::overhearing(both),
            false => both(),
        };
        assert_eq!(kept.shelf.shelvings, 0, "种子 {seed}：不收的那一趟收过");
        assert!(
            shelved.shelf.asked == kept.shelf.asked,
            "种子 {seed}：收过再放回的请求和一直不收的不一样"
        );
        let lines =
            |watch: &Watch| -> Vec<String> { watch.log().iter().map(Event::to_line).collect() };
        assert_eq!(lines(&shelved), lines(&kept), "种子 {seed}：日志不一样");
        shelvings += shelved.shelf.shelvings;
        quiet += shelved.shelf.quiet;
    }
    (shelvings, quiet)
}

/// 平时跑的六十例：收过再放回的请求、日志和一直不收的一样，真收过。
#[test]
fn shelving_changes_nothing() {
    let (shelvings, _) = same_without_shelving(0..60, false);
    assert!(shelvings >= 200, "六十例只收了 {shelvings} 次");
}

/// 全量：接着往后跑两千例（CI 的长跑那一项用 `--ignored`、release 模式跑）。
#[test]
#[ignore = "长跑，CI 的长跑那一项用 --ignored 跑（施工 LW-10 下）"]
fn shelving_changes_nothing_for_longer() {
    let (shelvings, _) = same_without_shelving(60..2060, false);
    assert!(shelvings >= 6_000, "两千例只收了 {shelvings} 次");
}

/// 闲着时插旁听的话、`events.append` 的六十例（施工 LW-36）：收着时来的不放回、直接收下，落了盘的并进槽里，请求、日志照样和
/// 一直不收的一样；真收过，收着时也真直接收下过。
#[test]
fn overhearing_while_shelved_changes_nothing() {
    let (shelvings, quiet) = same_without_shelving(0..60, true);
    assert!(shelvings >= 200, "六十例只收了 {shelvings} 次");
    assert!(quiet >= 50, "六十例收着时只直接收下了 {quiet} 条");
}

/// 全量：插旁听的话的接着往后跑两千例（CI 的长跑那一项用 `--ignored`、release 模式跑）。
#[test]
#[ignore = "长跑，CI 的长跑那一项用 --ignored 跑（施工 LW-36）"]
fn overhearing_while_shelved_changes_nothing_for_longer() {
    let (shelvings, quiet) = same_without_shelving(60..2060, true);
    assert!(shelvings >= 6_000, "两千例只收了 {shelvings} 次");
    assert!(quiet >= 1_500, "两千例收着时只直接收下了 {quiet} 条");
}
