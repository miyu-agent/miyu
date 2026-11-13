//! 内存紧不紧的判法：cgroup 快撞上限的紧；整台机器紧、核心是大头的紧，核心占得少的不紧；正好 5%、读不到的不紧；读的那一下
//! 崩了的不紧；表空着的不读。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use miyu_heap::room::{Room, Rooms};

use super::{LOW_PERCENT, Pressure, tight};
use crate::sessions::Sessions;

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// 100 GiB 里只剩 3 GiB：已用 97 GiB。
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

#[test]
fn a_cgroup_near_its_limit_is_tight_however_little_the_core_holds() {
    // bm 的样子：限 18 GiB、只剩 0.5 GiB；系统宽裕。
    let cgroup = Room {
        available: GIB / 2,
        total: 18 * GIB,
    };
    let found = tight(rooms(Some(PLENTY), Some(cgroup)), || Some(100 * MIB));
    assert_eq!(found, Some(cgroup));
    // cgroup 宽裕的照系统的判。
    let roomy = Room {
        available: 9 * GIB,
        total: 18 * GIB,
    };
    assert_eq!(tight(rooms(Some(PLENTY), Some(roomy)), || None), None);
}

#[test]
fn a_tight_machine_is_tight_only_when_the_core_holds_most_of_it() {
    // 核心占了 60 GiB：已用的一大半。
    assert_eq!(
        tight(rooms(Some(SYSTEM_LOW), None), || Some(60 * GIB)),
        Some(SYSTEM_LOW)
    );
    // 正好一半的算。
    let half = 97 * GIB / 2;
    assert_eq!(
        tight(rooms(Some(SYSTEM_LOW), None), || Some(half)),
        Some(SYSTEM_LOW)
    );
    assert_eq!(
        tight(rooms(Some(SYSTEM_LOW), None), || Some(half - 1)),
        None
    );
    // 别的程序吃满、核心才几百 MB：不紧。
    assert_eq!(
        tight(rooms(Some(SYSTEM_LOW), None), || Some(300 * MIB)),
        None
    );
    // 自己占多少读不到的：不知道，不紧。
    assert_eq!(tight(rooms(Some(SYSTEM_LOW), None), || None), None);
}

#[test]
fn exactly_the_threshold_and_unknown_are_not_tight() {
    let edge = Room {
        available: LOW_PERCENT,
        total: 100,
    };
    assert_eq!(tight(rooms(Some(edge), Some(edge)), || Some(100)), None);
    assert_eq!(tight(Rooms::default(), || Some(u64::MAX)), None);
}

#[test]
fn the_core_is_only_measured_when_the_machine_is_tight() {
    let measured = std::cell::Cell::new(false);
    let measure = || {
        measured.set(true);
        Some(0)
    };
    assert_eq!(tight(rooms(Some(PLENTY), None), measure), None);
    assert!(!measured.get(), "系统宽裕的不读自己");
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
    assert_eq!(sessions.pressure.tight().await, None);
}

#[tokio::test]
async fn an_empty_table_is_not_read() {
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&reads);
    let sessions = sessions(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        rooms(Some(SYSTEM_LOW), None)
    });
    assert!(!sessions.relieve(&mut None).await, "表空着");
    assert_eq!(reads.load(Ordering::SeqCst), 0, "空着的不读");
}
