//! 本机时区（施工 LW-5，`internal/blueprint/heap.md`「本机时区」）：运行日志每一行的时刻、核心起来那一行的 `tz`、会话的
//! 环境里的时区偏移照它算。
//!
//! 不直接用 `jiff::tz::TimeZone::system()`（`jiff::Zoned::now()` 走的就是它）：它每次都先打开 jiff 的 zoneinfo 库，库第一次
//! 打开时把 `/usr/share/zoneinfo` 整个目录走一遍，一千多个时区名连同完整路径留在堆上，核心一直不放（2026-11-21 量过：活着的
//! 堆里有 111 kB，见施工单 `LW-5-分配器余量.md`）。所以没设 `TZ`、`TZDIR` 的 Unix 上直接读 `/etc/localtime`：jiff 照它的链接
//! 认出名字以后在库里读的是同一份文件，链接认不出名字的 jiff 也是直接读它，读出来的规则一样，写出来的时刻和偏移一个字不差
//! （`zone/tests.rs` 新旧对照）。时区名照链接认（认不出的写 `Local`），只在印时区名的写法（`%Q`、`%V`、`Zoned` 的
//! `Display`）里看得到，用到这里的地方都不印。
//!
//! 别的情况一律照旧交给 jiff：设了 `TZ` 的（要照名字找）、设了 `TZDIR` 的（库在别处，读的可能是另一份）、`/etc/localtime`
//! 读不出来或者不是时区文件的（jiff 记警告、当 UTC 那一套照旧）、不是 Unix 的。读到的留 5 分钟（和 jiff 自己留的一样久），
//! 过了重读一次：核心开着时改了本机时区，照旧 5 分钟以内跟上。
//!
//! 定时任务照名字记时区（`miyu-endpoint` 的 `schedules.rs`），名字要和 jiff 库里的对得上，不走这里。

use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use jiff::Zoned;
use jiff::tz::TimeZone;

/// 本机时区的文件。
const LOCALTIME: &str = "/etc/localtime";

/// 链接里时区名前面的那一段：jiff 照它从链接里认名字（`jiff::tz::system` 的 Unix 那一份）。
const ZONEINFO: &str = "zoneinfo/";

/// 读到的留多久：和 jiff 自己留本机时区的一样久（`jiff::tz::system` 的 `TTL`）。
const TTL: Duration = Duration::from_secs(5 * 60);

/// 上一次读到的、什么时候读的。`None` 是这一次交给 jiff。
static CACHE: Mutex<Option<(Option<TimeZone>, Instant)>> = Mutex::new(None);

/// 本机时区：见这一页开头。
pub fn local() -> TimeZone {
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    let read = match &*cache {
        Some((read, at)) if at.elapsed() < TTL => read.clone(),
        _ => {
            let read = own().then(|| from_file(Path::new(LOCALTIME))).flatten();
            *cache = Some((read.clone(), Instant::now()));
            read
        }
    };
    drop(cache);
    read.unwrap_or_else(TimeZone::system)
}

/// 现在，照本机时区（[`local`]）。
pub fn now() -> Zoned {
    jiff::Timestamp::now().to_zoned(local())
}

/// 这里自己读不读：Unix 上、没设 `TZ` 和 `TZDIR` 的才读。
fn own() -> bool {
    cfg!(unix) && std::env::var_os("TZ").is_none() && std::env::var_os("TZDIR").is_none()
}

/// 照时区文件 `path` 认：读不出来、不是时区文件的没有（调的一方交给 jiff）。名字照链接里 `zoneinfo/` 后面的那一段，和 jiff
/// 认的一样；不是链接、链接里没有 `zoneinfo/` 的叫 `Local`。
fn from_file(path: &Path) -> Option<TimeZone> {
    let bytes = std::fs::read(path).ok()?;
    let target = std::fs::read_link(path).ok();
    let name = target
        .as_deref()
        .and_then(Path::to_str)
        .and_then(|target| {
            target
                .rfind(ZONEINFO)
                .map(|at| &target[at + ZONEINFO.len()..])
        })
        .unwrap_or("Local");
    TimeZone::tzif(name, &bytes).ok()
}

#[cfg(test)]
mod tests;
