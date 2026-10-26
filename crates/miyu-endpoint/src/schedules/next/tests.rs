//! 下一次（施工 J-1 上）：cron 只认五段，照给的时区算、不含现在这一刻；每隔一阵的照上一次往后数，错过的几个跳过；某个时刻
//! 一次的晚于现在才有、跑过就没了。时区写死成固定偏移，不随跑测试的机器变。

use jiff::tz::{Offset, TimeZone};

use super::*;

/// 东九区。
fn tokyo() -> TimeZone {
    TimeZone::fixed(Offset::constant(9))
}

/// 东九区 2026-10-26 的 `hh:mm`。
fn local(hh: i8, mm: i8) -> Timestamp {
    let zoned = jiff::civil::date(2026, 10, 26)
        .at(hh, mm, 0, 0)
        .to_zoned(tokyo())
        .unwrap();
    Timestamp::from_unix_millis(zoned.timestamp().as_millisecond()).unwrap()
}

fn cron(text: &str, recurring: bool) -> When {
    When::Cron {
        cron: text.to_string(),
        recurring,
    }
}

#[test]
fn cron_takes_five_fields_only() {
    assert!(parse("0 8 * * *").is_ok());
    assert!(parse("*/15 9-17 * * MON-FRI").is_ok());
    assert!(parse("0 0 8 * * *").is_err(), "六段（带秒）不认");
    assert!(parse("0 8 * *").is_err(), "四段不认");
    assert!(parse("61 8 * * *").is_err());
    assert!(parse("").is_err());
}

#[test]
fn cron_counts_in_the_zone_and_skips_the_current_minute() {
    let daily = cron("0 8 * * *", true);
    assert_eq!(first(&daily, local(7, 59), &tokyo()), Some(local(8, 0)));
    let next_day = jiff::civil::date(2026, 10, 27)
        .at(8, 0, 0, 0)
        .to_zoned(tokyo())
        .unwrap();
    let tomorrow = Timestamp::from_unix_millis(next_day.timestamp().as_millisecond()).unwrap();
    assert_eq!(
        first(&daily, local(8, 0), &tokyo()),
        Some(tomorrow),
        "不含现在"
    );
    assert_eq!(
        after(&daily, local(8, 0), local(8, 0), &tokyo()),
        Some(tomorrow)
    );
    assert_eq!(
        after(&daily, local(8, 0), local(20, 0), &tokyo()),
        Some(tomorrow),
        "错过的照现在往后找"
    );
    assert_eq!(first(&cron("not cron", true), local(8, 0), &tokyo()), None);
}

#[test]
fn every_steps_from_the_last_one_and_skips_missed_ones() {
    let every = When::Every { every: 600 };
    assert_eq!(first(&every, local(8, 0), &tokyo()), Some(local(8, 10)));
    assert_eq!(
        after(&every, local(8, 10), local(8, 10), &tokyo()),
        Some(local(8, 20))
    );
    assert_eq!(
        after(&every, local(8, 10), local(8, 45), &tokyo()),
        Some(local(8, 50)),
        "错过的 8:20、8:30、8:40 跳过"
    );
    assert_eq!(
        after(&every, local(8, 10), local(8, 50), &tokyo()),
        Some(local(9, 0)),
        "正好是那一刻的也算过去了"
    );
}

#[test]
fn a_one_off_is_there_only_while_it_is_ahead() {
    let at = When::At { at: local(9, 0) };
    assert_eq!(first(&at, local(8, 0), &tokyo()), Some(local(9, 0)));
    assert_eq!(first(&at, local(9, 0), &tokyo()), None);
    assert_eq!(after(&at, local(9, 0), local(9, 0), &tokyo()), None);
    assert!(once(&at));
    assert!(once(&cron("0 8 * * *", false)));
    assert!(!once(&cron("0 8 * * *", true)));
    assert!(!once(&When::Every { every: 60 }));
}
