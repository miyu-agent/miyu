//! 叫停的口子：登记了再举、举了再登记都调一次；举两次只调一次。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

fn counter() -> (Arc<AtomicUsize>, impl FnOnce() + Send + 'static) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    (count, move || {
        seen.fetch_add(1, Ordering::SeqCst);
    })
}

#[test]
fn registering_then_halting_calls_once() {
    let halt = Halt::new();
    let (count, then) = counter();
    halt.on_halt(then);
    assert_eq!(count.load(Ordering::SeqCst), 0, "没举起来不调");
    halt.halt();
    halt.halt();
    assert_eq!(count.load(Ordering::SeqCst), 1, "举两次只调一次");
}

#[test]
fn halting_before_registering_calls_at_once() {
    let halt = Halt::new();
    halt.halt();
    let (count, then) = counter();
    halt.clone().on_halt(then);
    assert_eq!(count.load(Ordering::SeqCst), 1, "举过了，登记就调");
}
