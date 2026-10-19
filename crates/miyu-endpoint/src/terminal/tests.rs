//! 闲置多久算久（施工 8-33）：照最后一次动静算，0 是不换，正好到线的不算。

use super::*;

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("时刻写对了")
}

#[test]
fn idle_counts_minutes_since_the_last_event() {
    let last = at(1_000_000);
    assert!(
        !idle(last, 240, at(1_000_000 + 240 * 60_000)),
        "正好到线的不算"
    );
    assert!(idle(last, 240, at(1_000_000 + 240 * 60_000 + 1)));
    assert!(!idle(last, 0, at(10_000_000_000_000)), "0 是不换");
}
