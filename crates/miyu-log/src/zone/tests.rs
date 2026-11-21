//! 本机时区新旧对照（施工 LW-5）：这里读出来的和 jiff 照系统认的（`TimeZone::system()`，原来 `Zoned::now()` 走的那一路）
//! 写出来一样；读不出来的交给 jiff。

use std::path::PathBuf;

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

/// 用到本机时区的几种写法：运行日志一行的时刻、起来那一行的 `tz`，和会话的环境里的偏移（照分钟算，`%:z` 写出来一样它就一样）。
const FORMATS: [&str; 2] = ["%Y-%m-%d %H:%M:%S%.3f", "%:z"];

/// 2026 年开头起，每隔 22 小时一个、一共四百多天：夏令时前后都取得到。
fn moments() -> impl Iterator<Item = Timestamp> {
    let start = Timestamp::from_second(1_767_225_600).expect("在范围里");
    (0..450).map(move |step| {
        start
            .checked_add(SignedDuration::from_hours(step * 22))
            .expect("在范围里")
    })
}

/// 两个时区在 [`moments`] 的每一刻、[`FORMATS`] 的每一种写法下写出来一样。
fn same(new: &TimeZone, old: &TimeZone) {
    for at in moments() {
        for format in FORMATS {
            assert_eq!(
                at.to_zoned(new.clone()).strftime(format).to_string(),
                at.to_zoned(old.clone()).strftime(format).to_string(),
                "{at} {format}"
            );
        }
    }
}

/// 这台机器的本机时区：新的读法和 jiff 照系统认的写出来一样（设了 `TZ` 的机器上新的本来就交给 jiff）。
#[test]
fn the_local_zone_writes_the_same_as_the_system_one() {
    same(&super::local(), &TimeZone::system());
}

/// 现在：照本机时区，偏移和 jiff 照系统认的一样。
#[test]
fn now_is_in_the_local_zone() {
    let now = super::now();
    let old = now.timestamp().to_zoned(TimeZone::system());
    assert_eq!(now.offset(), old.offset());
}

/// 测试自己的临时文件，丢掉时删。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        Scratch(std::env::temp_dir().join(format!("miyu-log-zone-{}-{name}", std::process::id())))
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "没建过的、删不掉的就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// 读不出来的（没有这个文件）没有：调的一方交给 jiff，照旧是 jiff 的那一套。
#[test]
fn a_missing_file_is_left_to_jiff() {
    let missing = Scratch::new("missing");
    assert!(!missing.0.exists());
    assert!(super::from_file(&missing.0).is_none());
}

/// 不是时区文件的没有：同上。
#[test]
fn a_file_that_is_not_a_zone_is_left_to_jiff() {
    let junk = Scratch::new("junk");
    std::fs::write(&junk.0, b"not a tzif file").expect("写得进");
    assert!(super::from_file(&junk.0).is_none());
}

/// 别处一份照抄的本机时区文件（不是链接）：照它认，写出来和 jiff 照系统认的一样，名字是 `Local`。没有 `/etc/localtime`、
/// 设了 `TZ` 的机器（照 `TZ` 认的和这份文件不一定是同一个时区）跳过。
#[test]
fn a_plain_copy_reads_the_same() {
    if std::env::var_os("TZ").is_some() {
        return;
    }
    let Ok(bytes) = std::fs::read(super::LOCALTIME) else {
        return;
    };
    let copy = Scratch::new("copy");
    std::fs::write(&copy.0, bytes).expect("写得进");
    let zone = super::from_file(&copy.0).expect("是时区文件");
    assert_eq!(zone.iana_name(), Some("Local"));
    same(&zone, &TimeZone::system());
}

/// 链到 zoneinfo 里的：名字照链接里 `zoneinfo/` 后面的那一段，和 jiff 认的一样。没有 `/etc/localtime` 的、它不是链到
/// zoneinfo 里的、设了 `TZ` 的机器跳过。
#[cfg(unix)]
#[test]
fn a_link_into_zoneinfo_is_named_like_jiff() {
    if std::env::var_os("TZ").is_some() || std::env::var_os("TZDIR").is_some() {
        return;
    }
    let Ok(target) = std::fs::read_link(super::LOCALTIME) else {
        return;
    };
    let Some(name) = target
        .to_str()
        .and_then(|target| target.rsplit_once(super::ZONEINFO))
        .map(|(_, name)| name.to_string())
    else {
        return;
    };
    // 相对的链接照 `/etc` 认：搬到临时目录里要写成绝对的，名字照旧从原来的链接里认。
    let target = std::path::Path::new(super::LOCALTIME)
        .parent()
        .map_or_else(|| target.clone(), |etc| etc.join(&target));
    let link = Scratch::new("link");
    std::os::unix::fs::symlink(&target, &link.0).expect("建得了链接");
    let zone = super::from_file(&link.0).expect("是时区文件");
    assert_eq!(zone.iana_name(), Some(name.as_str()));
    assert_eq!(zone.iana_name(), TimeZone::system().iana_name());
}
