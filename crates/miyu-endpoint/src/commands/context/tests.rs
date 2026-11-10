//! `/context` 的参数和约数（施工 CTX-2）：纯数字、`k`、`m`（大小写都行）、`unset`、`off`；别的写法、装不下的 `bad_params`；约数往下取。

use super::{about, parsed};
use crate::refusal::Refusal;

#[test]
fn numbers_with_k_and_m_and_off_are_read() {
    assert_eq!(parsed("128000").ok(), Some(Some(128_000)));
    assert_eq!(parsed("128k").ok(), Some(Some(128_000)));
    assert_eq!(parsed("128K").ok(), Some(Some(128_000)));
    assert_eq!(parsed("1m").ok(), Some(Some(1_000_000)));
    assert_eq!(parsed("2M").ok(), Some(Some(2_000_000)));
    assert_eq!(
        parsed("0").ok(),
        Some(Some(0)),
        "0 照数值读，太小由压缩线那一关拒"
    );
    assert_eq!(parsed("off").ok(), Some(None));
    assert_eq!(parsed("OFF").ok(), Some(None));
    assert_eq!(parsed("unset").ok(), Some(None));
    assert_eq!(parsed("UNSET").ok(), Some(None));
}

#[test]
fn other_spellings_are_bad_params() {
    for wrong in [
        "k",
        "m",
        "1.5m",
        "-1",
        "+5",
        "12 k",
        "128kb",
        "1km",
        "abc",
        "on",
        "1e6",
        "１２８",
        "99999999999999999999",
        "18446744073709552k",
    ] {
        assert!(
            matches!(parsed(wrong), Err(refusal) if refusal.reason == Refusal::BAD_PARAMS.reason),
            "{wrong}"
        );
    }
}

#[test]
fn about_rounds_down_to_k_and_m() {
    assert_eq!(about(0), "0");
    assert_eq!(about(999), "999");
    assert_eq!(about(1_000), "1k");
    assert_eq!(about(101_600), "101k");
    assert_eq!(about(128_000), "128k");
    assert_eq!(about(999_999), "999k");
    assert_eq!(about(1_000_000), "1m");
    assert_eq!(about(1_048_576), "1m");
    assert_eq!(about(1_500_000), "1.5m");
    assert_eq!(about(2_000_000), "2m");
}
