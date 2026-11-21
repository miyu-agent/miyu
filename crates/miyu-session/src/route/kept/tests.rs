//! 没有主请求满 [`KEEP`] 放掉留着的历史（施工 LW-3）：照 tokio 暂停的时钟拨，不真等。请求照金样那一批（`exchange/golden.rs`），
//! 四个驱动各一份。

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use miyu_drivers::Memo;
use miyu_kernel::id::ContentHash;

use super::{KEEP, Kept};
use crate::route::exchange::golden::{Setup, cases};
use crate::route::exchange::samples::{Media, media};

/// 金样那一批要的 blob，整份的字节。
fn blobs(media: &Media) -> BTreeMap<ContentHash, Vec<u8>> {
    [&media.png, &media.jpeg, &media.pdf, &media.notes]
        .into_iter()
        .map(|bytes| (ContentHash::of(bytes), bytes.clone()))
        .collect()
}

/// 照主请求的路编一次（[`Kept::encode`]），交回编好的。
fn encode(
    kept: &Arc<Kept>,
    setup: &Setup,
    blobs: &BTreeMap<ContentHash, Vec<u8>>,
) -> miyu_drivers::Encoded<miyu_drivers::Fragments> {
    let (driver, call, request, _) = setup;
    kept.encode(|memo| driver.fragments_memo(request, call, blobs, memo))
        .expect("编得了")
}

/// 拨钟 `by`，再让醒了的任务走完。
async fn pass(by: Duration) {
    tokio::time::advance(by).await;
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
}

/// 一份编过一次、留着的。
fn filled() -> (Arc<Kept>, Setup, BTreeMap<ContentHash, Vec<u8>>) {
    let media = media();
    let setup = cases(&media).remove(0);
    let blobs = blobs(&media);
    let kept = Arc::new(Kept::default());
    encode(&kept, &setup, &blobs);
    (kept, setup, blobs)
}

#[tokio::test(start_paused = true)]
async fn what_was_used_less_than_keep_ago_is_kept() {
    let (kept, _, _) = filled();
    assert!(!kept.is_empty(), "编过以后留着");
    pass(KEEP - Duration::from_millis(1)).await;
    assert!(!kept.is_empty(), "不到 3 分钟还留着");
    assert!(kept.waiting(), "定时还在等");
}

#[tokio::test(start_paused = true)]
async fn what_was_unused_for_keep_is_released() {
    let (kept, _, _) = filled();
    pass(KEEP).await;
    assert!(kept.is_empty(), "满 3 分钟放掉了");
    assert!(!kept.waiting(), "放掉以后没有定时在等");
}

#[tokio::test(start_paused = true)]
async fn another_request_within_keep_counts_from_itself() {
    let (kept, setup, blobs) = filled();
    pass(Duration::from_secs(120)).await;
    encode(&kept, &setup, &blobs);
    pass(KEEP - Duration::from_secs(120)).await;
    assert!(!kept.is_empty(), "到头一次的时刻不放：后来又用过");
    assert!(kept.waiting(), "接着等到新的时刻");
    pass(Duration::from_secs(120) - Duration::from_millis(1)).await;
    assert!(!kept.is_empty(), "离后一次不到 3 分钟还留着");
    pass(Duration::from_millis(1)).await;
    assert!(kept.is_empty(), "离后一次满 3 分钟放掉了");
}

/// 放掉以后再编：和一直留着接着编的、整份编的一模一样（片段清单、每条的位置、路径都一样，发出去的字节就一样），四个驱动都是。
#[tokio::test(start_paused = true)]
async fn after_release_the_same_request_encodes_to_the_same_bytes() {
    let media = media();
    let blobs = blobs(&media);
    for setup in cases(&media) {
        let (driver, call, request, _) = &setup;
        let kept = Arc::new(Kept::default());
        let first = encode(&kept, &setup, &blobs);
        pass(KEEP).await;
        assert!(kept.is_empty(), "放掉了");
        let again = encode(&kept, &setup, &blobs);
        assert!(!kept.is_empty(), "又留下了");
        assert!(kept.waiting(), "又挂了一个定时");
        let mut memo = Memo::new();
        driver
            .fragments_memo(request, call, &blobs, &mut memo)
            .expect("编得了");
        let warm = driver
            .fragments_memo(request, call, &blobs, &mut memo)
            .expect("编得了");
        let whole = driver.fragments(request, call, &blobs).expect("编得了");
        assert_eq!(again, first, "放掉以后从头编的和头一次的一样");
        assert_eq!(again, warm, "和一直留着接着编的一样");
        assert_eq!(again, whole, "和整份编的一样");
    }
}

#[tokio::test(start_paused = true)]
async fn the_timer_does_not_keep_a_dropped_cache_alive() {
    let (kept, _, _) = filled();
    let weak: Weak<Kept> = Arc::downgrade(&kept);
    drop(kept);
    assert!(weak.upgrade().is_none(), "定时只拿着弱引用");
    pass(KEEP).await;
    assert!(weak.upgrade().is_none(), "到点醒来也没把它拿回来");
}

#[tokio::test(start_paused = true)]
async fn nothing_waits_before_the_first_request() {
    let kept = Arc::new(Kept::default());
    assert!(!kept.waiting(), "没编过的没有定时");
    pass(KEEP * 2).await;
    assert!(kept.is_empty());
    assert!(!kept.waiting());
}
