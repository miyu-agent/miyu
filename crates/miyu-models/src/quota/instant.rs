//! 供应商回的时刻（施工 QU-1）：照 RFC 3339 的写法读，带不带小数秒、`Z` 还是时区偏移都认，换成 UTC 的 [`Timestamp`]。
//! 日期和时刻之间认 `T`、`t`、空格；小数秒只取到毫秒，多的舍掉；闰秒（`60` 秒）不认。

use miyu_kernel::time::Timestamp;

/// 读 `2026-11-14T08:31:28.000Z`、`2026-11-14T08:31:28Z`、`2026-11-14T17:31:28+09:00` 这类写法；读不出的是空的。
pub(super) fn rfc3339(text: &str) -> Option<Timestamp> {
    if !text.is_ascii() || text.len() < 20 {
        return None;
    }
    let (date, rest) = text.split_at(10);
    let (separator, rest) = rest.split_at(1);
    if !matches!(separator, "T" | "t" | " ") {
        return None;
    }
    let (clock, mut rest) = rest.split_at(8);
    let mut millis = "000".to_string();
    if let Some(after) = rest.strip_prefix('.') {
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        millis = format!("{:0<3}", &after[..digits.min(3)]);
        rest = &after[digits..];
    }
    let offset = offset_minutes(rest)?;
    let utc_as_written = Timestamp::parse(&format!("{date}T{clock}.{millis}Z")).ok()?;
    Timestamp::from_unix_millis(utc_as_written.unix_millis() - offset * 60_000)
}

/// `Z`、`z` 是 0；`+HH:MM`、`-HH:MM` 是带符号的分钟数。
fn offset_minutes(text: &str) -> Option<i64> {
    if matches!(text, "Z" | "z") {
        return Some(0);
    }
    let (sign, rest) = match text.split_at_checked(1)? {
        ("+", rest) => (1, rest),
        ("-", rest) => (-1, rest),
        _ => return None,
    };
    let (hours, minutes) = rest.split_once(':')?;
    let number = |part: &str, most: i64| {
        (part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| part.parse::<i64>().ok())
            .flatten()
            .filter(|value| *value <= most)
    };
    Some(sign * (number(hours, 23)? * 60 + number(minutes, 59)?))
}
