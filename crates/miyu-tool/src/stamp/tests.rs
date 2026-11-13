use std::time::{Duration, UNIX_EPOCH};

use super::at;

/// 1970 年以后的截掉不到一毫秒的零头。
#[test]
fn after_the_epoch_drops_the_fraction() {
    let time = UNIX_EPOCH + Duration::from_nanos(1_234_999_999);
    assert_eq!(at(time).map(|at| at.unix_millis()), Some(1_234));
}

/// 1970 年以前的也往早取：差不到 1 毫秒的都算前一毫秒，整毫秒的照原样。差 100 纳秒：Windows 的时刻精度是 100 纳秒，
/// 差 1 纳秒在那里表示不出来、截回 1970 年那一刻（SW-1 补，CI 只 Windows 红过）。
#[test]
fn before_the_epoch_rounds_earlier() {
    let just = UNIX_EPOCH - Duration::from_nanos(100);
    assert_eq!(at(just).map(|at| at.unix_millis()), Some(-1));
    let whole = UNIX_EPOCH - Duration::from_millis(5);
    assert_eq!(at(whole).map(|at| at.unix_millis()), Some(-5));
}

/// 9999 年以后的写不成时刻：没有。
#[test]
fn too_late_is_none() {
    let late = UNIX_EPOCH + Duration::from_secs(300_000_000_000);
    assert_eq!(at(late), None);
}
