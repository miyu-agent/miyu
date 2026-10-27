//! 一个会话的抽取状态（`Extractor`）：闹钟作废、在路上的只有一个、同一段失败三次才放过。

use super::*;
use crate::memory::extract::tests::seq;

#[test]
fn an_alarm_is_void_once_another_is_set_or_a_turn_starts() {
    let extractor = Extractor::default();
    let first = extractor.arm();
    assert!(extractor.due(first));
    let second = extractor.arm();
    assert!(!extractor.due(first), "上了新的，旧的作废");
    extractor.cancel();
    assert!(!extractor.due(second), "忙起来了");
    let third = extractor.arm();
    assert!(extractor.start(third));
    assert!(!extractor.due(third), "在路上的只有一个");
    assert!(!extractor.start(third));
    assert_eq!(extractor.finish(seq(1), true), 0);
    assert!(extractor.due(third));
    extractor.start(third);
    extractor.abandon();
    assert!(extractor.due(third), "读不成的放下，不算失败");
}

#[test]
fn the_same_slice_failing_three_times_is_let_go() {
    let extractor = Extractor::default();
    let failed = |after: u64| {
        let generation = extractor.arm();
        assert!(extractor.start(generation));
        extractor.finish(seq(after), false)
    };
    assert_eq!(failed(5), 1);
    assert_eq!(failed(5), 2);
    assert_eq!(failed(9), 1, "换了一段重新数");
    assert_eq!(failed(9), 2);
    assert_eq!(failed(9), 3);
    assert_eq!(failed(9), 1, "放过以后重新数");
}

#[test]
fn an_alarm_is_pending_until_it_starts_or_is_let_go() {
    let extractor = Extractor::default();
    assert!(!extractor.pending(), "没上过闹钟");
    let first = extractor.arm();
    assert!(extractor.pending(), "上了，还没响");
    extractor.disarm(first);
    assert!(!extractor.pending(), "响了、不抽，放下了");
    let second = extractor.arm();
    extractor.disarm(first);
    assert!(extractor.pending(), "作废的那一个放下不算数");
    assert!(extractor.start(second));
    assert!(extractor.pending(), "在路上");
    extractor.finish(seq(1), true);
    assert!(!extractor.pending(), "抽完了");
    extractor.arm();
    extractor.cancel();
    assert!(!extractor.pending(), "忙起来撤掉了");
    let third = extractor.arm();
    extractor.start(third);
    extractor.abandon();
    assert!(!extractor.pending(), "读不成的放下了");
}
