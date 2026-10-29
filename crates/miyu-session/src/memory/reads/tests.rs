//! 回合开始问记忆的那几次读的登记（施工 TMP-1 补3）：等的时候登记着的都读完了才交回；没有登记着的当场交回。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::Reads;

#[test]
fn settling_waits_for_every_read_that_began() {
    let reads = Arc::new(Reads::default());
    let finished = Arc::new(AtomicBool::new(false));
    let (first, second) = (reads.begin(), reads.begin());
    let seen = Arc::clone(&finished);
    let reading = std::thread::spawn(move || {
        drop(first);
        std::thread::sleep(Duration::from_millis(200));
        seen.store(true, Ordering::SeqCst);
        drop(second);
    });
    reads.settle();
    assert!(finished.load(Ordering::SeqCst), "两次都读完了才交回");
    reading.join().expect("没 panic");
}

#[test]
fn settling_with_nothing_reading_returns_at_once() {
    let reads = Arc::new(Reads::default());
    drop(reads.begin());
    let started = Instant::now();
    reads.settle();
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "读完了的不等"
    );
}
