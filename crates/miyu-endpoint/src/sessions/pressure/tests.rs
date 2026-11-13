//! 内存紧不紧的判法：门槛照总量（施工 MEM-4）；cgroup 快撞上限的紧；整台机器紧、核心是大头的紧，核心占得少的不紧；正好等于
//! 门槛、读不到的不紧；离门槛不到两倍的近，近的、紧的隔 1 秒再看，远的隔 10 秒；读的那一下崩了的不紧；表空着的不读。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use miyu_heap::room::{Room, Rooms};

use super::{EVERY, Gauge, NEAR_EVERY, Pressure, judge, threshold};
use crate::sessions::Sessions;

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// 100 GiB 里只剩 3 GiB：已用 97 GiB。门槛 5 GiB。
const SYSTEM_LOW: Room = Room {
    available: 3 * GIB,
    total: 100 * GIB,
};

/// 宽裕：一半。
const PLENTY: Room = Room {
    available: 50 * GIB,
    total: 100 * GIB,
};

fn rooms(system: Option<Room>, cgroup: Option<Room>) -> Rooms {
    Rooms { system, cgroup }
}

/// 限 18 GiB 的 cgroup，还剩 `available`。
fn bm(available: u64) -> Room {
    Room {
        available,
        total: 18 * GIB,
    }
}

#[test]
fn the_threshold_is_a_tenth_at_most_two_gib_never_below_a_twentieth() {
    // 1.5 GiB：10% 是 153.6 MiB。
    assert_eq!(threshold(3 * GIB / 2), 3 * GIB / 2 / 10);
    assert_eq!(threshold(3 * GIB / 2) / MIB, 153);
    // 18 GiB：10% 是 1843.2 MiB，往下取整到字节。
    assert_eq!(threshold(18 * GIB), 18 * GIB / 10);
    assert_eq!(threshold(18 * GIB) / MIB, 1843);
    // 20 GiB 起 2 GiB 封顶；40 GiB 正好也是 5%。
    assert_eq!(threshold(20 * GIB), 2 * GIB);
    assert_eq!(threshold(30 * GIB), 2 * GIB);
    assert_eq!(threshold(40 * GIB), 2 * GIB);
    // 再大照 5%：100 GiB 是 5 GiB。
    assert_eq!(threshold(100 * GIB), 5 * GIB);
    // 两头不溢出。
    assert_eq!(threshold(0), 0);
    assert_eq!(threshold(u64::MAX), u64::MAX / 20);
}

#[test]
fn a_cgroup_near_its_limit_is_tight_however_little_the_core_holds() {
    // bm 的样子：限 18 GiB、只剩 0.5 GiB；系统宽裕。
    let low = bm(GIB / 2);
    let found = judge(rooms(Some(PLENTY), Some(low)), || Some(100 * MIB));
    assert_eq!(found, Gauge::Tight(low));
    // 施工 MEM-4：还剩 1.7 GiB（9%）照旧的 5% 不紧，照 10% 紧。
    let tenth = bm(17 * GIB / 10);
    assert_eq!(
        judge(rooms(Some(PLENTY), Some(tenth)), || None),
        Gauge::Tight(tenth)
    );
    // cgroup 宽裕的照系统的判。
    assert_eq!(
        judge(rooms(Some(PLENTY), Some(bm(9 * GIB))), || None),
        Gauge::Far
    );
}

#[test]
fn within_twice_the_threshold_is_near() {
    let line = threshold(18 * GIB);
    for (available, want) in [
        (2 * line, Gauge::Far),
        (2 * line - 1, Gauge::Near),
        (line, Gauge::Near),
        (line - 1, Gauge::Tight(bm(line - 1))),
    ] {
        let found = judge(rooms(Some(PLENTY), Some(bm(available))), || None);
        assert_eq!(found, want, "还剩 {available}");
    }
    // 整台机器近线（100 GiB 里剩 7 GiB，门槛 5 GiB）：核心是大头的近，别的程序吃的远。
    let near = Room {
        available: 7 * GIB,
        total: 100 * GIB,
    };
    assert_eq!(
        judge(rooms(Some(near), None), || Some(60 * GIB)),
        Gauge::Near
    );
    assert_eq!(
        judge(rooms(Some(near), None), || Some(300 * MIB)),
        Gauge::Far
    );
    // 一边近、一边紧的照紧的；一边近、一边远的照近的。
    assert_eq!(
        judge(rooms(Some(SYSTEM_LOW), Some(bm(2 * line - 1))), || Some(
            60 * GIB
        )),
        Gauge::Tight(SYSTEM_LOW)
    );
    assert_eq!(
        judge(rooms(Some(PLENTY), Some(bm(2 * line - 1))), || None),
        Gauge::Near
    );
}

#[test]
fn a_tight_machine_is_tight_only_when_the_core_holds_most_of_it() {
    // 核心占了 60 GiB：已用的一大半。
    assert_eq!(
        judge(rooms(Some(SYSTEM_LOW), None), || Some(60 * GIB)),
        Gauge::Tight(SYSTEM_LOW)
    );
    // 正好一半的算。
    let half = 97 * GIB / 2;
    assert_eq!(
        judge(rooms(Some(SYSTEM_LOW), None), || Some(half)),
        Gauge::Tight(SYSTEM_LOW)
    );
    assert_eq!(
        judge(rooms(Some(SYSTEM_LOW), None), || Some(half - 1)),
        Gauge::Far
    );
    // 别的程序吃满、核心才几百 MB：不紧。
    assert_eq!(
        judge(rooms(Some(SYSTEM_LOW), None), || Some(300 * MIB)),
        Gauge::Far
    );
    // 自己占多少读不到的：不知道，不紧。
    assert_eq!(judge(rooms(Some(SYSTEM_LOW), None), || None), Gauge::Far);
}

#[test]
fn exactly_the_threshold_and_unknown_are_not_tight() {
    let edge = Room {
        available: threshold(100),
        total: 100,
    };
    assert_eq!(
        judge(rooms(Some(edge), Some(edge)), || Some(100)),
        Gauge::Near
    );
    assert_eq!(judge(Rooms::default(), || Some(u64::MAX)), Gauge::Far);
}

#[test]
fn the_core_is_only_measured_when_the_machine_is_near() {
    let measured = std::cell::Cell::new(false);
    let measure = || {
        measured.set(true);
        Some(0)
    };
    assert_eq!(judge(rooms(Some(PLENTY), None), measure), Gauge::Far);
    assert!(!measured.get(), "系统离线还远的不读自己");
}

/// 一份看内存的，读数照给的，隔多久照出厂。
fn pressure(rooms: impl Fn() -> Rooms + Send + Sync + 'static) -> Pressure {
    Pressure::new(
        Arc::new(rooms),
        Arc::new(|| Some(60 * GIB)),
        EVERY,
        Duration::ZERO,
    )
}

#[tokio::test]
async fn near_the_line_looks_every_second_and_backs_off_when_clear() {
    let now = Arc::new(Mutex::new(bm(9 * GIB)));
    let reading = Arc::clone(&now);
    let pressure = pressure(move || {
        let cgroup = *reading.lock().unwrap_or_else(PoisonError::into_inner);
        rooms(Some(PLENTY), Some(cgroup))
    });
    let line = threshold(18 * GIB);
    let mut waits = Vec::new();
    for available in [
        9 * GIB,
        2 * line - 1,
        line - 1,
        2 * line - 1,
        2 * line,
        9 * GIB,
    ] {
        *now.lock().unwrap_or_else(PoisonError::into_inner) = bm(available);
        waits.push(pressure.wait(pressure.look().await));
    }
    assert_eq!(
        waits,
        [EVERY, NEAR_EVERY, NEAR_EVERY, NEAR_EVERY, EVERY, EVERY],
        "远 10 秒、近和紧 1 秒、出来恢复"
    );
    // 测试里隔得比 1 秒短的，近的时候照它。
    let quick = Pressure {
        every: Duration::from_millis(20),
        ..pressure
    };
    assert_eq!(quick.wait(Gauge::Near), Duration::from_millis(20));
}

/// 一张会话表，读法照给的、计数。
fn sessions(rooms: impl Fn() -> Rooms + Send + Sync + 'static) -> Sessions {
    Sessions {
        pressure: Pressure::new(
            Arc::new(rooms),
            Arc::new(|| Some(60 * GIB)),
            Duration::from_secs(3600),
            Duration::ZERO,
        ),
        ..Sessions::default()
    }
}

#[tokio::test]
async fn a_probe_that_panics_is_not_tight() {
    let sessions = sessions(|| panic!("读的那一下崩了"));
    assert_eq!(sessions.pressure.look().await, Gauge::Far);
}

#[tokio::test]
async fn an_empty_table_is_not_read() {
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&reads);
    let sessions = sessions(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        rooms(Some(SYSTEM_LOW), None)
    });
    assert_eq!(sessions.relieve(&mut None).await, None, "表空着");
    assert_eq!(reads.load(Ordering::SeqCst), 0, "空着的不读");
}
