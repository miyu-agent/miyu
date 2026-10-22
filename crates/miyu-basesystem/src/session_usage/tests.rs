//! 金额给她看的写法（施工 8-15）：三位有效数字、至少两位小数，多出来的 0 去掉。

use super::money;

#[test]
fn money_keeps_three_significant_digits_and_at_least_two_decimals() {
    for (amount, written) in [
        (0.42, "0.42"),
        (1.3, "1.30"),
        (0.000_292_05, "0.000292"),
        (0.0123, "0.0123"),
        (0.5, "0.50"),
        (12.0, "12.00"),
        (1234.5678, "1234.57"),
        (0.0, "0.00"),
        (0.000_001_234, "0.00000123"),
    ] {
        assert_eq!(money(amount), written, "{amount}");
    }
}

/// 用时写成时分秒，零的那一截不写（施工 E-M 下）。
#[test]
fn working_time_is_hours_minutes_seconds() {
    for (ms, written) in [
        (0, "0s"),
        (999, "0s"),
        (45_000, "45s"),
        (123_000, "2m 3s"),
        (3_605_000, "1h 0m 5s"),
        (90_061_000, "25h 1m 1s"),
    ] {
        assert_eq!(super::hms(ms), written, "{ms}");
    }
}
