//! 开会话之前那一道：门槛的两边、不知道的放过、读数留一秒、读的那一下崩了的当不知道。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use miyu_heap::room::Room;

use super::{Gate, LOW_PERCENT, Probe};
use crate::sessions::Sessions;

const GIB: u64 = 1024 * 1024 * 1024;

/// 一张会话表，读数照 `probe`、留 `fresh`。
fn sessions(probe: Probe, fresh: Duration) -> Sessions {
    Sessions {
        gate: Gate::new(probe, fresh),
        ..Sessions::default()
    }
}

fn fixed(room: Option<Room>) -> Sessions {
    sessions(Arc::new(move || room), Duration::ZERO)
}

#[tokio::test]
async fn below_the_threshold_is_refused_with_what_is_left() {
    let left = Room {
        available: 3 * GIB,
        total: 100 * GIB,
    };
    let refused = fixed(Some(left)).room().await.expect_err("只剩 3% 要拒");
    assert_eq!(refused.reason, "low_memory");
    let data = refused.data.expect("带着剩多少");
    assert_eq!(data["available_mib"], 3072);
    assert_eq!(data["available_percent"], 3);
}

#[tokio::test]
async fn exactly_the_threshold_and_unknown_are_let_through() {
    let edge = Room {
        available: LOW_PERCENT,
        total: 100,
    };
    assert_eq!(fixed(Some(edge)).room().await, Ok(()));
    assert_eq!(fixed(None).room().await, Ok(()));
}

#[tokio::test]
async fn a_reading_is_kept_for_a_while() {
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&reads);
    let probe: Probe = Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        None
    });
    let kept = sessions(Arc::clone(&probe), Duration::from_secs(60));
    for _ in 0..3 {
        assert_eq!(kept.room().await, Ok(()));
    }
    assert_eq!(reads.load(Ordering::SeqCst), 1, "一分钟里只读一次");
    let fresh = sessions(probe, Duration::ZERO);
    for _ in 0..3 {
        assert_eq!(fresh.room().await, Ok(()));
    }
    assert_eq!(reads.load(Ordering::SeqCst), 4, "不留的每次都读");
}

#[tokio::test]
async fn a_probe_that_panics_is_unknown() {
    let probe: Probe = Arc::new(|| panic!("读的那一下崩了"));
    assert_eq!(sessions(probe, Duration::ZERO).room().await, Ok(()));
}

#[tokio::test]
async fn a_child_is_told_why_in_english() {
    let left = Room {
        available: GIB / 2,
        total: 18 * GIB,
    };
    let why = fixed(Some(left)).room_for_child().await.expect_err("要拒");
    assert_eq!(
        why,
        "Only 512 MB (2%) of memory available. Can't open a session now. Free some memory and try again."
    );
}
