//! 安静下来还内存（施工 V-5 中）：一阵请求只还一次，没请求的时候不还。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use std::sync::Arc;

use super::*;

/// 安静下来以后什么都不再做。
fn nothing() -> Then {
    Box::new(|| Box::pin(async {}))
}

static RELEASED: AtomicUsize = AtomicUsize::new(0);

fn release() -> bool {
    RELEASED.fetch_add(1, Ordering::SeqCst);
    true
}

/// 等到还过 `times` 次，最多等两秒；交回等到了没有。
async fn released(times: usize) -> bool {
    let since = Instant::now();
    while since.elapsed() < Duration::from_secs(2) {
        if RELEASED.load(Ordering::SeqCst) >= times {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    false
}

#[tokio::test]
async fn a_burst_of_requests_is_settled_once_and_quiet_stays_asleep() {
    let quiet = Duration::from_millis(60);
    let activity = Arc::new(Activity::default());
    let task = spawn(&activity, quiet, release, nothing());
    // 没请求：一直不还。
    tokio::time::sleep(quiet * 3).await;
    assert_eq!(RELEASED.load(Ordering::SeqCst), 0, "没请求不还");
    // 一阵请求，中间的间隔都比安静的门槛短：最后一个过去以后还一次。
    for _ in 0..5 {
        activity.touch();
        tokio::time::sleep(quiet / 3).await;
    }
    assert_eq!(RELEASED.load(Ordering::SeqCst), 0, "还在一阵里，不还");
    assert!(released(1).await, "安静下来还一次");
    tokio::time::sleep(quiet * 3).await;
    assert_eq!(RELEASED.load(Ordering::SeqCst), 1, "之后没请求，不再还");
    // 再来一个，再还一次。
    activity.touch();
    assert!(released(2).await, "下一阵过去再还一次");
    task.abort();
}

/// 还内存的那一次被取消了（施工 BUG-1：停核心时运行时在收尾，阻塞任务排不上）：不当成崩了打 `ERROR`，不再接着盯；真 panic 的才
/// 打 `ERROR heap settle panicked`，接着盯。
#[tokio::test]
async fn a_cancelled_settle_at_shutdown_is_not_reported_as_a_panic() {
    use miyu_log::{LevelFilter, Memory};
    let cancelled = {
        let task = tokio::spawn(std::future::pending::<bool>());
        task.abort();
        task.await.expect_err("叫停了")
    };
    let panicked = tokio::spawn(async { panic!("boom") })
        .await
        .map(|()| true)
        .expect_err("崩了");
    let memory = Memory::new();
    let (after_cancel, after_panic) = tracing::subscriber::with_default(
        miyu_log::subscriber(memory.clone(), LevelFilter::DEBUG, None),
        || (settled(Err(cancelled)), settled(Err(panicked))),
    );
    assert!(!after_cancel, "取消的不再盯");
    assert!(after_panic, "崩了的接着盯");
    let lines = memory.lines();
    let errors: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains(" ERROR "))
        .collect();
    assert_eq!(errors.len(), 1, "{lines:?}");
    assert!(
        errors[0].contains("heap settle panicked panic=true"),
        "{lines:?}"
    );
}
