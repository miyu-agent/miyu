//! 访问密码握手的限流（施工 5-20；AS-4，发布前审计 10-30，项目主人同一天定的数）：
//!
//! - 照来源数：60 秒内错 5 次的，剩下的时间里先拒、不验。来源是网页软件转来的 `connection.remote` 的地址，本机来的算一份
//!   `local`。地址去掉端口；IPv6 照前 64 位合成一份（一台机器手里通常就是一整段 /64，换个地址不该重新数），IPv4 映射成
//!   IPv6 写法的照 IPv4 算。读不成地址的照原样当一份。
//! - 全局：所有来源合起来 60 秒内 30 次，挡换着来源猜的。
//! - 查和记在同一把锁里：同时进来的握手都看到「还没错满」、都去验的话，挡不住并发，每个还各占一份 argon2 的内存（施工
//!   5-20）。所以能试的当场先记一次，验对了再撤掉（[`Throttle::forgive`]）。
//! - 过了窗口的记录每次都清掉，没有记录的来源连键一起去掉：表的大小不随见过多少个地址涨。

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// 数失败的窗口。
const WINDOW: Duration = Duration::from_secs(60);

/// 一个来源窗口里错几次就先拒（设计 06 U4 的初值）。
const TRIES: usize = 5;

/// 所有来源合起来窗口里几次就先拒（施工 AS-4）。
const ALL_TRIES: usize = 30;

/// 核心一份。
#[derive(Default)]
pub(super) struct Throttle {
    counts: Mutex<Counts>,
}

/// 窗口里记着的：照来源的、合起来的。都照先后排。
#[derive(Default)]
struct Counts {
    by_source: BTreeMap<String, Vec<Instant>>,
    all: Vec<Instant>,
}

impl Throttle {
    /// 来源 `source` 能不能再试：能的当场记一次，交回记下的时刻（验对了交给 [`Throttle::forgive`]）；这个来源、或者合起来已经
    /// 满了的，交回还要等多久（等到挡住它的那一次满 60 秒，两样都满的取长的）。
    pub(super) fn admit(&self, source: &str) -> Result<Instant, Duration> {
        self.admit_at(source, Instant::now())
    }

    /// 同 [`Throttle::admit`]，此刻是 `now`。
    fn admit_at(&self, source: &str, now: Instant) -> Result<Instant, Duration> {
        let mut counts = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        let fresh = |at: &Instant| now.saturating_duration_since(*at) < WINDOW;
        counts.all.retain(fresh);
        counts.by_source.retain(|_, tries| {
            tries.retain(fresh);
            !tries.is_empty()
        });
        let key = group(source);
        let mine = counts.by_source.get(&key).map_or(&[][..], Vec::as_slice);
        let wait = [
            blocked(mine, TRIES, now),
            blocked(&counts.all, ALL_TRIES, now),
        ]
        .into_iter()
        .flatten()
        .max();
        if let Some(wait) = wait {
            return Err(wait);
        }
        counts.by_source.entry(key).or_default().push(now);
        counts.all.push(now);
        Ok(now)
    }

    /// 来源 `source` 在 `at` 那一次验对了：清掉这个来源错的次数，合起来的那一份撤掉这一次。
    pub(super) fn forgive(&self, source: &str, at: Instant) {
        let mut counts = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        counts.by_source.remove(&group(source));
        if let Some(index) = counts.all.iter().position(|tried| *tried == at) {
            counts.all.remove(index);
        }
    }
}

/// 窗口里的 `tries` 满了 `most` 次的，交回还要等多久：等到倒数第 `most` 次满窗口。
fn blocked(tries: &[Instant], most: usize, now: Instant) -> Option<Duration> {
    let first = tries.len().checked_sub(most).map(|index| tries[index])?;
    Some(WINDOW.saturating_sub(now.saturating_duration_since(first)))
}

/// 来源记在哪一份：地址去掉端口，IPv6 照前 64 位，IPv4 映射的照 IPv4，读不成的照原样。
fn group(source: &str) -> String {
    let address = source
        .parse::<SocketAddr>()
        .map(|socket| socket.ip())
        .or_else(|_| {
            source
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<IpAddr>()
        });
    match address {
        Ok(IpAddr::V4(v4)) => v4.to_string(),
        Ok(IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let [a, b, c, d, ..] = v6.segments();
                format!("{a:x}:{b:x}:{c:x}:{d:x}::/64")
            }
        },
        Err(_) => source.to_string(),
    }
}

#[cfg(test)]
mod tests;
