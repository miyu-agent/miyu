//! 回答里的 `sent` 交给模型资料记着（施工 MD-3）：同一个请求再来、编出了不同的字节，查得出上一次报的那一份。

use super::*;

#[tokio::test]
async fn the_reported_sent_is_kept_for_the_next_comparison() {
    let fake = scripted(vec![Plan::Answer(hello(), Duration::ZERO, usage())]);
    let ready = ready(&fake, Duration::from_secs(5));
    let Speaker::Extension(reached) = &ready.speaker else {
        unreachable!()
    };
    let blobs = Blobs::new(std::env::temp_dir().join("miyu-ext-driver-unused"));
    let mut request_hash = None;
    exchange(
        &ready,
        reached,
        Cow::Owned(request(None)),
        &blobs,
        std::future::pending(),
        |progress| {
            if let Progress::Sent { request, .. } = progress {
                request_hash = Some(request);
            }
        },
    )
    .await;
    let request_hash = request_hash.expect("发出去了");
    let reported = ContentHash::parse(&format!("sha256:{}", "ab".repeat(32))).expect("哈希");
    assert_eq!(
        ready
            .learn
            .data
            .wires()
            .note("pi/ag", &request_hash, &ContentHash::of(b"different")),
        Some(reported),
        "同一个请求编出了不同的字节：交回上一次报的"
    );
}
