//! 下一次是什么时候（施工 J-1 上，设计 `45-定时和循环.md` 第三节第 1 条）：cron 照标准五段写法、照记下的时区算（施工 J-3 上，以前建的照本机的）（croner，夏令时照
//! OCPS 规范：跳过的那一小时里的时点挪到跳过以后，重复的那一小时只算一次）；某个时刻一次的就是那一刻；每隔一阵的照上一次往后
//! 数。错过的不补：照现在往后找。

use croner::Cron;
use croner::parser::{CronParser, Seconds};
use jiff::tz::TimeZone;

use miyu_kernel::time::Timestamp;
use miyu_store::schedules::When;

/// 隔一阵一次的最短间隔（秒）：同她自己定节奏的循环的下限（设计 45 第三节第 4 条）。
pub const MIN_EVERY: u64 = 60;

/// 读一句 cron：只认五段（分、时、日、月、星期），不认秒、年。读不成的交回为什么（英文，原样给人看）。
///
/// # Errors
///
/// 不是五段、哪一段写得不对。
pub fn parse(cron: &str) -> Result<Cron, String> {
    CronParser::builder()
        .seconds(Seconds::Disallowed)
        .build()
        .parse(cron)
        .map_err(|error| error.to_string())
}

/// 比 `now` 晚的第一次（不含 `now`）：cron 照 `zone` 算；某个时刻一次的晚于 `now` 才有；每隔一阵的从 `now` 往后一个间隔。
/// 算不出的（cron 读不成、往后再没有了）没有。
pub fn first(when: &When, now: Timestamp, zone: &TimeZone) -> Option<Timestamp> {
    match when {
        When::Cron {
            cron, zone: named, ..
        } => cron_after(cron, now, &read_in(named, zone)),
        When::At { at } => (*at > now).then_some(*at),
        When::Every { every } => plus(now, *every),
    }
}

/// 照 `due` 那一次跑过以后，比 `now` 晚的下一次：每隔一阵的照 `due` 往后数整数个间隔（错过的那几个跳过），cron 照 `now`
/// 往后找；某个时刻一次的没有下一次。
pub fn after(when: &When, due: Timestamp, now: Timestamp, zone: &TimeZone) -> Option<Timestamp> {
    match when {
        When::Cron {
            cron, zone: named, ..
        } => cron_after(cron, now, &read_in(named, zone)),
        When::At { .. } => None,
        When::Every { every } => {
            let step = i64::try_from(every.checked_mul(1000)?).ok()?;
            if step <= 0 {
                return None;
            }
            let behind = now.unix_millis().checked_sub(due.unix_millis())?.max(0);
            let steps = behind / step + 1;
            Timestamp::from_unix_millis(due.unix_millis().checked_add(steps.checked_mul(step)?)?)
        }
    }
}

/// 错过的最多数到几个：每分钟一次的停了一天也不用一个一个数完。
const MISSED_CAP: u32 = 1000;

/// 从 `due` 那一次（含）到 `now`（含）本该响几次（施工 J-4）：核心停着时错过的个数，最多数到 [`MISSED_CAP`]。`due` 还没到的
/// 是零。
pub fn missed(when: &When, due: Timestamp, now: Timestamp, zone: &TimeZone) -> u32 {
    if due > now {
        return 0;
    }
    match when {
        When::At { .. } => 1,
        When::Every { every } => {
            let Some(step) = every
                .checked_mul(1000)
                .and_then(|step| i64::try_from(step).ok())
            else {
                return 1;
            };
            if step <= 0 {
                return 1;
            }
            let behind = now.unix_millis().saturating_sub(due.unix_millis());
            u32::try_from(behind / step + 1).map_or(MISSED_CAP, |count| count.min(MISSED_CAP))
        }
        When::Cron {
            cron, zone: named, ..
        } => {
            let zone = read_in(named, zone);
            let mut count = 1;
            let mut at = due;
            while count < MISSED_CAP {
                match cron_after(cron, at, &zone) {
                    Some(later) if later <= now => {
                        count += 1;
                        at = later;
                    }
                    _ => break,
                }
            }
            count
        }
    }
}

/// 跑完是不是就没了：某个时刻一次的、不循环的 cron。
pub fn once(when: &When) -> bool {
    matches!(
        when,
        When::At { .. }
            | When::Cron {
                recurring: false,
                ..
            }
    )
}

/// cron `cron` 比 `now` 晚的第一次，照 `zone` 算。
fn cron_after(cron: &str, now: Timestamp, zone: &TimeZone) -> Option<Timestamp> {
    let cron = parse(cron).ok()?;
    let start = jiff::Timestamp::from_millisecond(now.unix_millis())
        .ok()?
        .to_zoned(zone.clone());
    let next = cron.find_next_occurrence(&start, false).ok()?;
    Timestamp::from_unix_millis(next.timestamp().as_millisecond())
}

/// cron 照哪个时区读（施工 J-3 上）：记下的那个，读不成（以前建的没有、名字不认识了）的照 `fallback`。
fn read_in(named: &Option<String>, fallback: &TimeZone) -> TimeZone {
    named
        .as_deref()
        .and_then(|name| TimeZone::get(name).ok())
        .unwrap_or_else(|| fallback.clone())
}

/// `at` 往后 `seconds` 秒。
fn plus(at: Timestamp, seconds: u64) -> Option<Timestamp> {
    let millis = i64::try_from(seconds.checked_mul(1000)?).ok()?;
    Timestamp::from_unix_millis(at.unix_millis().checked_add(millis)?)
}

#[cfg(test)]
mod tests;
