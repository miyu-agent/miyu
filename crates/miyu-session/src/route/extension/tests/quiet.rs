//! 驱动声明了多久没动静才算断（施工 DR-2，清单 `quiet_secs`）：比这一轮的空闲时限长的照它等，等第一个字、两段之间都算；没声明的
//! 照空闲时限。实测 agy 的 medium 档每次 100～150 秒才出第一个字（不流思考），离 180 秒很近。时钟停着（施工 RB-1）：
//! 200 秒照假的时钟跳过去，不真等。

use super::*;

/// 照 `fake` 发一次，空闲时限 `idle`，驱动声明的是 `quiet`。
async fn run_quiet(
    fake: Arc<Fake>,
    idle: Duration,
    quiet: Option<Duration>,
) -> (Exchanged, Vec<Delta>) {
    let mut ready = ready(&fake, idle);
    let Speaker::Extension(reached) = &mut ready.speaker else {
        unreachable!()
    };
    reached.reach.quiet = quiet;
    let Speaker::Extension(reached) = &ready.speaker else {
        unreachable!()
    };
    let blobs = Blobs::new(std::env::temp_dir().join("miyu-ext-driver-unused"));
    let mut deltas = Vec::new();
    let ended = exchange(
        &ready,
        reached,
        Cow::Owned(request(None)),
        &blobs,
        std::future::pending(),
        |progress| {
            if let Progress::Delta(delta) = progress {
                deltas.push(delta);
            }
        },
    )
    .await;
    (ended, deltas)
}

/// 每段都隔 200 秒到（第一个字也是）：空闲时限 180 秒，声明 300 秒的说完，没声明的第一个字没等到就按卡住断了。
#[tokio::test(start_paused = true)]
async fn a_driver_that_declares_a_longer_quiet_is_waited_for() {
    let slow = || {
        scripted(vec![Plan::Answer(
            hello(),
            Duration::from_secs(200),
            usage(),
        )])
    };
    let idle = Duration::from_secs(180);
    let (ended, deltas) = run_quiet(slow(), idle, Some(Duration::from_secs(300))).await;
    assert_eq!(error(&ended), None, "声明了 300 秒的照等");
    assert_eq!(deltas.len(), 4, "两份四段全交了");
    let (ended, deltas) = run_quiet(slow(), idle, None).await;
    assert_eq!(
        error(&ended),
        Some((ErrorClass::Stalled, "the driver went quiet".to_string())),
        "没声明的照 180 秒断"
    );
    assert!(deltas.is_empty(), "第一个字没等到");
    // 声明得比空闲时限短的不收紧：空闲时限 300 秒、声明 60 秒，照 300 秒等。
    let (ended, _) = run_quiet(
        slow(),
        Duration::from_secs(300),
        Some(Duration::from_secs(60)),
    )
    .await;
    assert_eq!(error(&ended), None, "短的不收紧");
}
