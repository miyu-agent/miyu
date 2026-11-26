use std::sync::atomic::AtomicUsize;

use super::*;

/// 叫了几次。
fn counter() -> (std::sync::Arc<AtomicUsize>, impl Fn() + Send + Sync) {
    let told = std::sync::Arc::new(AtomicUsize::new(0));
    let counted = std::sync::Arc::clone(&told);
    (told, move || {
        counted.fetch_add(1, Ordering::Relaxed);
    })
}

#[test]
fn the_default_tells_after_three_seconds_and_gives_up_after_ten_minutes() {
    // 2026-11-27 项目主人定。
    assert_eq!(Patience::DEFAULT.slow, Duration::from_secs(3));
    assert_eq!(Patience::DEFAULT.limit, Duration::from_secs(600));
}

/// 一直不好的：过了 `slow` 叫一次，只叫一次；到了上限交回没有。
#[tokio::test(start_paused = true)]
async fn a_wait_that_never_ends_is_told_once_then_given_up() {
    let (told, slow) = counter();
    let patience = Patience {
        slow: Duration::from_secs(3),
        limit: Duration::from_secs(600),
    };
    let started = Instant::now();
    let waited = patiently(std::future::pending::<()>(), patience, &slow).await;
    assert_eq!(waited, None);
    assert_eq!(started.elapsed(), Duration::from_secs(600));
    assert_eq!(told.load(Ordering::Relaxed), 1);
}

/// 好得快的：不叫，交回它的结果。
#[tokio::test(start_paused = true)]
async fn a_quick_wait_is_not_told() {
    let (told, slow) = counter();
    let quick = async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        7
    };
    assert_eq!(patiently(quick, Patience::DEFAULT, &slow).await, Some(7));
    assert_eq!(told.load(Ordering::Relaxed), 0);
}

/// 几段等共用一个钟：叫过一次以后再看也不叫。
#[tokio::test(start_paused = true)]
async fn one_clock_tells_once_across_its_waits() {
    let (told, slow) = counter();
    let clock = Clock::new(Patience::DEFAULT, &slow);
    for _ in 0..100 {
        clock.pause().await;
    }
    assert_eq!(told.load(Ordering::Relaxed), 1, "过了 3 秒叫了一次");
    assert!(!clock.expired());
    let waited = clock
        .within(tokio::time::sleep(Duration::from_secs(1)))
        .await;
    assert_eq!(waited, Some(()));
    assert_eq!(told.load(Ordering::Relaxed), 1, "不再叫");
}
